use std::cell::RefCell;
use std::rc::Rc;

use anyhow::{Context, Result};
use everscale_types::models::{
    BlockchainConfigParams, CurrencyCollection, ExtInMsgInfo, IntMsgInfo, MsgInfo, OwnedMessage,
    SimpleLib, StdAddr,
};
use everscale_types::num::Tokens;
use everscale_types::prelude::*;
use everscale_vm::{
    BehaviourModifiers, CustomSmcInfo, GasParams, SmcInfo, SmcInfoBase, SmcInfoTonV6, Stack, Tuple,
    VmState, VmVersion,
};
use num_bigint::BigInt;

pub struct TvmEmulator {
    pub code: Cell,
    pub data: Cell,
    pub args: Args,
}

impl TvmEmulator {
    pub fn new(code: Cell, data: Cell) -> Self {
        Self {
            code,
            data,
            args: Default::default(),
        }
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
                max: 1000000,
                limit,
                credit,
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
        stack.items.push(Rc::new(BigInt::from(method_id)));

        // Prepare VM state
        let mut b = VmState::builder()
            .with_raw_stack(Rc::new(stack))
            .with_code(self.code.clone())
            .with_data(self.data.clone())
            .with_smc_info(self.args.build_smc_info(self.code.clone()))
            .with_libraries(self.args.collect_libraries())
            .with_gas(self.args.gas_params.unwrap_or_else(GasParams::getter))
            .with_modifiers(BehaviourModifiers {
                chksig_always_succeed: self.args.ignore_chksig,
                ..Default::default()
            });

        let mut debug_output = None;
        if self.args.debug_enabled {
            let writer = DebugOutput::default();
            debug_output = Some(writer.buffer.clone());
            b = b.with_debug(writer);
        }

        let mut vm = b.build();

        // Run VM
        let exit_code = !vm.run();

        // Parse VM output
        let stack = vm.stack.clone();
        let gas_used = vm.gas.gas_consumed();
        let accepted = vm.gas.gas_credit == 0;

        let mut actions = None;

        let code = self.code.clone();
        let mut data = self.data.clone();
        if accepted {
            if let Some(commited) = vm.commited_state.take() {
                data = commited.c4;
                actions = Some(commited.c5);
            }
        }

        drop(vm);

        let vm_log = match debug_output {
            None => String::new(),
            Some(output) => Rc::unwrap_or_clone(output).into_inner(),
        };

        Answer {
            code,
            data,
            accepted,
            stack,
            actions,
            exit_code,
            gas_used,
            vm_log,
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

    pub fn set_gas_limit(&mut self, gas_limit: i64) {
        self.args.gas_params = Some(GasParams {
            max: u64::MAX,
            limit: gas_limit as u64,
            credit: 0,
        });
    }
}

pub struct Answer {
    pub code: Cell,
    pub data: Cell,
    pub accepted: bool,
    pub stack: Rc<Stack>,
    pub actions: Option<Cell>,
    pub exit_code: i32,
    pub gas_used: u64,
    pub vm_log: String,
}

#[derive(Default)]
pub struct Args {
    pub gas_params: Option<GasParams>,
    pub raw_c7: Option<Rc<Tuple>>,
    pub now: Option<u32>,
    pub rand_seed: Option<HashBytes>,
    pub ignore_chksig: bool,
    pub amount: u64,
    pub balance: u64,
    pub debug_enabled: bool,

    pub address: Option<StdAddr>,
    pub config: Option<ParsedConfig>,
    pub libraries: Option<Dict<HashBytes, SimpleLib>>,
    pub prev_blocks_info: Option<Rc<Tuple>>,
}

impl Args {
    fn collect_libraries(&self) -> Vec<Dict<HashBytes, SimpleLib>> {
        // TODO: Add global libraries.
        let mut libraries = Vec::new();
        libraries.extend(self.libraries.clone());
        libraries
    }

    fn build_smc_info(&self, code: Cell) -> Box<dyn SmcInfo> {
        if let Some(c7) = self.raw_c7.clone() {
            return Box::new(CustomSmcInfo {
                version: VmVersion::LATEST_TON,
                c7,
            });
        }

        let now = self.now.unwrap_or_default();

        let mut b = SmcInfoBase::new()
            .with_now(now)
            .with_block_lt(0)
            .with_tx_lt(0)
            .with_raw_rand_seed(self.rand_seed.unwrap_or_default())
            .with_account_balance(CurrencyCollection::new(self.balance as _))
            .with_account_addr(self.address().into());

        let mut global_version = 1;
        let mut unpacked_config = None;
        if let Some(config) = &self.config {
            b = b.with_config(config.params.clone());

            global_version = config.version;

            if global_version >= 6 {
                unpacked_config = Some(config.unpacked.clone());
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

        Box::new(b)
    }

    fn build_stack(&self, message_amount: u64, message_body: Cell, selector: i32) -> Stack {
        let mut stack = Stack::default();

        // Account balance
        stack.items.push(Rc::new(BigInt::from(if self.balance > 0 {
            self.balance
        } else {
            10_000_000_000
        })));

        // Message balance
        stack.items.push(Rc::new(BigInt::from(message_amount)));

        // Message cell
        stack.items.push(Rc::new(if selector == 0 {
            self.build_internal_message(message_amount, message_body.clone())
        } else {
            self.build_external_message(message_body.clone())
        }));

        // Message body
        stack.items.push(Rc::new(message_body));

        // Done
        stack
    }

    fn build_internal_message(&self, amount: u64, body: Cell) -> Cell {
        let body_range = CellSliceRange::full(body.as_ref());

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
            body: (body, body_range),
            layout: None,
        })
        .unwrap()
    }

    fn build_external_message(&self, body: Cell) -> Cell {
        let body_range = CellSliceRange::full(body.as_ref());

        CellBuilder::build_from(OwnedMessage {
            info: MsgInfo::ExtIn(ExtInMsgInfo {
                src: None,
                dst: self.address().into(),
                import_fee: Tokens::ZERO,
            }),
            init: None,
            body: (body, body_range),
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
    pub unpacked: Rc<Tuple>,
    // TODO: Replace with VM version.
    pub version: u32,
}

impl ParsedConfig {
    pub fn try_from_root(root: Cell) -> Result<Self> {
        let params = BlockchainConfigParams::from_raw(root);

        let global = params
            .get_global_version()
            .context("Failed to get global version")?;

        let unpacked =
            SmcInfoTonV6::unpack_config(&params, 0).context("Failed to unpack config params")?;

        Ok(Self {
            params,
            unpacked,
            version: global.version,
        })
    }
}

#[derive(Default)]
struct DebugOutput {
    buffer: Rc<RefCell<String>>,
}

impl std::fmt::Write for DebugOutput {
    fn write_char(&mut self, c: char) -> std::fmt::Result {
        self.buffer.borrow_mut().push(c);
        Ok(())
    }

    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.buffer.borrow_mut().push_str(s);
        Ok(())
    }
}
