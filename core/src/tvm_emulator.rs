use anyhow::{Context, Result};
use num_bigint::BigInt;
use tycho_types::models::{
    BlockchainConfigParams, CurrencyCollection, ExtInMsgInfo, ExtraCurrencyCollection,
    GlobalCapability, IntMsgInfo, MsgInfo, OwnedMessage, SimpleLib, StdAddr,
};
use tycho_types::num::Tokens;
use tycho_types::prelude::*;
use tycho_vm::{
    BehaviourModifiers, CustomSmcInfo, GasParams, SafeRc, SmcInfo, SmcInfoBase, SmcInfoTonV6,
    Stack, Tuple, VmState, VmVersion, tuple,
};

use crate::subscriber::VmLogSubscriber;
use crate::util::make_vm_log_mask;

const MAX_GAS: u64 = 1_000_000;
const BASE_GAS_PRICE: u64 = 1000 << 16;

pub struct TvmEmulator {
    pub code: Cell,
    pub data: Cell,
    pub args: Args,
}

impl TvmEmulator {
    pub fn new(code: Cell, data: Cell, verbosity: i32) -> Self {
        Self {
            code,
            data,
            args: Args {
                verbosity,
                ..Default::default()
            },
        }
    }

    pub fn make_logger(&self) -> VmLogSubscriber {
        let mut log_max_size = 256;
        if self.args.verbosity > 4 {
            log_max_size = 32 << 20;
        } else if self.args.verbosity > 0 {
            log_max_size = 1 << 20;
        }

        let mask = make_vm_log_mask(self.args.verbosity, false);
        VmLogSubscriber::new(mask, log_max_size)
    }

    pub fn send_external_message(&mut self, body: Cell) -> Answer {
        let stack = self.args.build_stack(0, body, -1);
        self.run_method(-1, stack)
    }

    pub fn send_internal_message(&mut self, body: Cell, amount: u64) -> Answer {
        let stack = self.args.build_stack(amount, body, 0);
        self.run_method(0, stack)
    }

    fn run_method(&mut self, method_id: i32, stack: Stack) -> Answer {
        let prev_gas_params = self.args.gas_params;
        if self.args.gas_params.is_none() {
            let is_internal = method_id == 0;
            let (limit, credit) = if is_internal {
                (self.args.amount.saturating_mul(1000), 0)
            } else {
                (0, 10000)
            };

            self.args.gas_params = Some(GasParams {
                max: MAX_GAS,
                limit,
                credit,
                price: BASE_GAS_PRICE,
            });
        }

        let res = self.run_get_method(method_id, stack);
        self.args.gas_params = prev_gas_params;
        self.code = res.code.clone();
        self.data = res.data.clone();

        res
    }

    pub fn run_get_method(&self, method_id: i32, mut stack: Stack) -> Answer {
        // Prepare stack
        stack
            .items
            .push(SafeRc::new_dyn_value(BigInt::from(method_id)));

        // Prepare VM state
        let (enable_signature_domains, signature_with_id) = self
            .args
            .config
            .as_ref()
            .map(|c| (c.enable_signature_domains, c.signature_with_id))
            .unwrap_or_default();

        let mut b = VmState::builder()
            .with_raw_stack(SafeRc::new(stack))
            .with_code(self.code.clone())
            .with_data(self.data.clone())
            .with_smc_info(self.args.build_smc_info(self.code.clone()))
            .with_libraries(&self.args.libraries)
            .with_gas(self.args.gas_params.unwrap_or_else(GasParams::getter))
            .with_init_selector(false)
            .with_modifiers(BehaviourModifiers {
                enable_signature_domains,
                signature_with_id,
                chksig_always_succeed: self.args.ignore_chksig,
                log_mask: make_vm_log_mask(self.args.verbosity, false),
                ..Default::default()
            });

        let mut debug_log = String::new();
        if self.args.debug_enabled {
            b = b.with_debug(&mut debug_log);
        }

        let mut vm = b.build();

        // Run VM
        let exit_code = !vm.run();

        // Parse VM output
        let stack = vm.stack.clone();
        let gas_used = vm.gas.consumed();
        let accepted = vm.gas.credit() == 0;

        let mut actions = None;

        let code = self.code.clone();
        let mut data = self.data.clone();
        if accepted && let Some(commited) = vm.committed_state.take() {
            data = commited.c4;
            actions = Some(commited.c5);
        }

        let missing_library = vm.gas.missing_library();

        drop(vm);

        Answer {
            code,
            data,
            accepted,
            stack,
            actions,
            exit_code,
            gas_used,
            debug_log,
            missing_library,
        }
    }

    pub fn set_c7(
        &mut self,
        address: StdAddr,
        unixtime: u32,
        balance: u64,
        rand_seed: &HashBytes,
        config: Option<ParsedConfig>,
    ) {
        self.args.address = Some(address);
        self.args.now = Some(unixtime);
        self.args.balance = balance;
        self.args.rand_seed = Some(*rand_seed);
        if config.is_some() {
            self.args.config = config;
        }
    }

    pub fn set_gas_limit(&mut self, gas_limit: u64) {
        self.args.gas_params = Some(GasParams {
            max: MAX_GAS,
            limit: gas_limit,
            credit: 0,
            price: BASE_GAS_PRICE,
        });
    }
}

pub struct Answer {
    pub code: Cell,
    pub data: Cell,
    pub accepted: bool,
    pub stack: SafeRc<Stack>,
    pub actions: Option<Cell>,
    pub exit_code: i32,
    pub gas_used: u64,
    pub debug_log: String,
    pub missing_library: Option<HashBytes>,
}

#[derive(Default)]
pub struct Args {
    pub gas_params: Option<GasParams>,
    pub raw_c7: Option<SafeRc<Tuple>>,
    pub now: Option<u32>,
    pub rand_seed: Option<HashBytes>,
    pub ignore_chksig: bool,
    pub amount: u64,
    pub balance: u64,
    pub extra: ExtraCurrencyCollection,
    pub verbosity: i32,
    pub debug_enabled: bool,

    pub address: Option<StdAddr>,
    pub config: Option<ParsedConfig>,
    pub libraries: Option<Dict<HashBytes, SimpleLib>>,
    pub prev_blocks_info: Option<SafeRc<Tuple>>,
}

impl Args {
    fn build_smc_info(&self, code: Cell) -> Box<dyn SmcInfo> {
        if let Some(c7) = self.raw_c7.clone() {
            return Box::new(CustomSmcInfo {
                version: VmVersion::LATEST_TON,
                c7,
            });
        }

        let now = self.now.unwrap_or_default();

        let balance = CurrencyCollection {
            tokens: Tokens::new(self.balance as _),
            other: self.extra.clone(),
        };

        let mut b = SmcInfoBase::new()
            .with_now(now)
            .with_block_lt(0)
            .with_tx_lt(0)
            .with_raw_rand_seed(self.rand_seed.unwrap_or_default())
            .with_account_balance(balance)
            .with_account_addr(self.address().into());

        let mut global_version = 1;
        let mut unpacked_config = None;
        if let Some(config) = &self.config {
            b = b.with_config(config.params.clone());

            global_version = config.version;

            if global_version >= 6 {
                unpacked_config = Some(
                    SmcInfoTonV6::unpack_config(&config.params, now)
                        .expect("parsed config must be valid"),
                );
            }
        }

        if global_version < 4 {
            return Box::new(b);
        }

        let mut b = b
            .require_ton_v4()
            .with_code(code)
            .with_message_balance(CurrencyCollection::ZERO)
            .with_storage_fees(Tokens::ZERO);
        if let Some(prev_blocks_info) = &self.prev_blocks_info {
            b = b.with_prev_blocks_info(prev_blocks_info.clone());
        }

        if global_version < 6 {
            return Box::new(b);
        }

        let mut b = b.require_ton_v6().with_due_payment(Tokens::ZERO);
        if let Some(unpacked_config) = unpacked_config {
            b = b.with_unpacked_config(unpacked_config);
        }

        if global_version < 11 {
            return Box::new(b);
        }

        let b = b.require_ton_v11().with_unpacked_in_msg(None);
        Box::new(b)
    }

    fn build_stack(&self, message_amount: u64, message_body: Cell, selector: i32) -> Stack {
        Stack {
            items: tuple![
                int if self.balance > 0 {
                    self.balance
                } else {
                    10_000_000_000
                },
                int message_amount,
                cell if selector == 0 {
                    self.build_internal_message(message_amount, message_body.clone())
                } else {
                    self.build_external_message(message_body.clone())
                },
                slice CellSliceParts::from(message_body),
            ],
        }
    }

    fn build_internal_message(&self, amount: u64, body: Cell) -> Cell {
        CellBuilder::build_from(OwnedMessage {
            info: MsgInfo::Int(IntMsgInfo {
                ihr_disabled: true,
                bounce: true,
                bounced: false,
                src: StdAddr::new(-1, HashBytes::ZERO).into(),
                dst: self.address().into(),
                value: CurrencyCollection::new(amount as _),
                ihr_fee: Tokens::ZERO,
                fwd_fee: Tokens::ZERO,
                created_lt: 0,
                created_at: 0,
            }),
            init: None,
            body: body.into(),
            layout: None,
        })
        .unwrap()
    }

    fn build_external_message(&self, body: Cell) -> Cell {
        CellBuilder::build_from(OwnedMessage {
            info: MsgInfo::ExtIn(ExtInMsgInfo {
                src: None,
                dst: self.address().into(),
                import_fee: Tokens::ZERO,
            }),
            init: None,
            body: body.into(),
            layout: None,
        })
        .unwrap()
    }

    fn address(&self) -> StdAddr {
        self.address
            .clone()
            .unwrap_or_else(|| StdAddr::new(0, HashBytes::ZERO))
    }
}

#[derive(Clone)]
pub struct ParsedConfig {
    pub params: BlockchainConfigParams,
    // TODO: Replace with VM version.
    pub version: u32,
    pub enable_signature_domains: bool,
    pub signature_with_id: Option<i32>,
}

impl ParsedConfig {
    pub fn try_from_root(root: Cell) -> Result<Self> {
        let params = BlockchainConfigParams::from_raw(root);

        // Try to unpack config to return error early.
        tycho_vm::SmcInfoTonV6::unpack_config(&params, 0)
            .context("Failed to unpack config params")?;

        let global = params
            .get_global_version()
            .context("Failed to get global version")?;

        let capabilities = global.capabilities;
        let enable_signature_domains = capabilities.contains(GlobalCapability::CapSignatureDomain);

        let signature_with_id = if enable_signature_domains
            || capabilities.contains(GlobalCapability::CapSignatureWithId)
        {
            params
                .get_global_id()
                .context("Global id is mandatory (param 19)")
                .map(Some)?
        } else {
            None
        };

        Ok(Self {
            params,
            version: global.version,
            enable_signature_domains,
            signature_with_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tycho_config() {
        let root = Boc::decode(include_bytes!("../res/tycho_config.boc")).unwrap();
        let config = ParsedConfig::try_from_root(root).unwrap();

        for item in config.params.as_dict().keys() {
            item.unwrap();
        }
    }

    #[test]
    fn parse_ton_config() {
        let root = Boc::decode(include_bytes!("../res/ton_config.boc")).unwrap();
        let config = ParsedConfig::try_from_root(root).unwrap();

        for item in config.params.as_dict().keys() {
            item.unwrap();
        }
    }
}
