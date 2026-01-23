use std::sync::OnceLock;

use anyhow::{Context, Result};
use tycho_types::models::{
    BlockchainConfigParams, IntAddr, LibDescr, MsgInfo, ShardAccount, SimpleLib, TickTock,
};
use tycho_types::prelude::*;
use tycho_vm::Stack;
use wasm_bindgen::prelude::*;

use crate::models::{
    EmulatorParams, ErrResponse, OkResponse, RunGetMethodParams, RunGetMethodResponse,
    TxEmulatorErrorResponse, TxEmulatorMsgNotAcceptedResponse, TxEmulatorResponse,
    TxEmulatorSuccessResponse, VersionInfo,
};
use crate::tvm_emulator::{self, TvmEmulator};
use crate::tx_emulator::TxEmulator;
use crate::util::{JsonBool, now_sec_u64};
use crate::{EMULATOR_COMMIT_DATE, EMULATOR_COMMIT_HASH};

// === Exported Methods ===

#[wasm_bindgen]
pub fn version() -> js_sys::JsString {
    static RESPONSE: OnceLock<String> = OnceLock::new();
    let info = RESPONSE.get_or_init(|| {
        serde_json::to_string(&VersionInfo {
            emulator_lib_commit_hash: EMULATOR_COMMIT_HASH,
            emulator_lib_commit_date: EMULATOR_COMMIT_DATE,
        })
        .unwrap()
    });
    JsValue::from_str(info.as_str()).unchecked_into()
}

#[wasm_bindgen]
pub fn create_emulator(config: &str, verbosity: i32) -> Result<*mut TxEmulator, JsError> {
    (|| {
        let config = Boc::decode_base64(config)?;
        let emulator = TxEmulator::new(BlockchainConfigParams::from_raw(config), verbosity)?;
        Ok::<_, anyhow::Error>(Box::into_raw(Box::new(emulator)))
    })()
    .map_err(|e| JsError::new(&e.to_string()))
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[wasm_bindgen]
pub fn destroy_emulator(transaction_emulator: *mut TxEmulator) -> Result<(), JsError> {
    if transaction_emulator.is_null() {
        return Err(JsError::new("transaction_emulator is null"));
    }

    _ = unsafe { Box::from_raw(transaction_emulator) };
    Ok(())
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[wasm_bindgen]
pub fn emulate_with_emulator(
    emulator: *mut TxEmulator,
    libs: Option<String>,
    account: &str,
    message: Option<String>,
    params: &str,
) -> js_sys::JsString {
    // Parse input params.
    let (emulator, params, libraries, mut prev_blocks_info) = match (|| {
        anyhow::ensure!(!emulator.is_null(), "emulator pointer is null");
        let emulator = unsafe { &mut *emulator };

        let params: EmulatorParams =
            serde_json::from_str(params).context("Failed to parse emulator params")?;
        anyhow::ensure!(
            params.is_tick_tock || !params.is_tock,
            "Inconsistent parameters: is_tick_tock=false, is_tock=true"
        );

        let libraries = if let Some(libs) = libs {
            let root = Boc::decode_base64(libs).context("Failed to deserialize libraries")?;
            emulator_libs_to_shard(root)?
        } else {
            Dict::new()
        };

        let prev_blocks = parse_prev_blocks_info(params.prev_blocks_info.as_ref())?;

        Ok((emulator, params, libraries, prev_blocks))
    })() {
        // Parsed input params.
        Ok(res) => res,
        // Fatal error on invalid input.
        Err(e) => {
            let value = serde_json::to_string(&ErrResponse {
                message: format!("{e:?}"),
            })
            .unwrap();
            return JsValue::from(value).unchecked_into();
        }
    };

    (move || {
        let subscriber = emulator.make_logger();
        let vm_log = subscriber.state().clone();
        let _tracing = tracing::subscriber::set_default(subscriber);

        // Parse accounts and messages.
        let is_tock = params.is_tock;
        let unixtime = if params.unixtime == 0 {
            now_sec_u64() as u32
        } else {
            params.unixtime
        };

        emulator
            .config
            .update_storage_prices(unixtime)
            .context("Failed to unpack storage prices")?;

        let account = Boc::decode_base64(account)
            .context("Failed to deserialize shard account")?
            .parse::<ShardAccount>()
            .context("Failed to unpack shard account")?;

        let is_external;
        let message = if let Some(msg) = message {
            anyhow::ensure!(
                !params.is_tick_tock,
                "Tick-tock transactions cannot have an inbound message"
            );

            let msg = Boc::decode_base64(msg).context("Failed to deserialize message")?;
            let msg_info = msg
                .parse::<MsgInfo>()
                .context("Failed to unpack message info")?;

            is_external = msg_info.is_external_in();
            Some((msg, msg_info))
        } else {
            is_external = false;
            anyhow::ensure!(
                params.is_tick_tock,
                "Inbound message is required for ordinary transactions"
            );
            None
        };

        let IntAddr::Std(address) =
            (match account.load_account().context("Failed to unpack account")? {
                Some(account) => account.address,
                None => match message.as_ref().map(|(_, info)| info) {
                    Some(MsgInfo::Int(info)) => info.dst.clone(),
                    Some(MsgInfo::ExtIn(info)) => info.dst.clone(),
                    Some(MsgInfo::ExtOut(_)) => {
                        anyhow::bail!("Only internal and external inbound messages are accepted");
                    }
                    None => anyhow::bail!("Can't run tick-tock transaction on account_none"),
                },
            })
        else {
            anyhow::bail!("var_addr is not supported");
        };

        if let Some(rand_seed) = params.rand_seed {
            emulator.rand_seed = rand_seed;
        }

        let debug_enabled = params.debug_enabled;
        let params = tycho_executor::ExecutorParams {
            libraries,
            rand_seed: emulator.rand_seed,
            block_unixtime: unixtime,
            block_lt: params.lt,
            // Will be overwritten by custom hook
            prev_mc_block_id: None,
            vm_modifiers: tycho_vm::BehaviourModifiers {
                chksig_always_succeed: params.ignore_chksig,
                ..emulator.vm_modifiers
            },
            disable_delete_frozen_accounts: params.disable_delete_frozen_accounts.unwrap_or(true),
            charge_action_fees_on_fail: params.charge_action_fees_on_fail.unwrap_or(true),
            full_body_in_bounced: params.full_body_in_bounced.unwrap_or(false),
            strict_extra_currency: params.strict_extra_currency.unwrap_or(true),
            authority_marks_enabled: params.authority_marks_enabled.unwrap_or(false),
        };

        let mut debug_log = String::new();
        let mut smc_info_hook = move |smc_info: &mut tycho_executor::phase::ComputePhaseSmcInfo| {
            smc_info.base.base.prev_blocks_info = prev_blocks_info.take();
            Ok(())
        };
        let mut inspector = tycho_executor::ExecutorInspector {
            debug: debug_enabled.then_some(&mut debug_log),
            modify_smc_info: Some(&mut smc_info_hook),
            ..Default::default()
        };

        let output = match message {
            Some((msg_root, _)) => tycho_executor::Executor::new(&params, &emulator.config)
                .with_min_lt(params.block_lt)
                .begin_ordinary_ext(
                    &address,
                    is_external,
                    msg_root,
                    &account,
                    Some(&mut inspector),
                ),
            None => {
                let ty = if is_tock {
                    TickTock::Tock
                } else {
                    TickTock::Tick
                };
                tycho_executor::Executor::new(&params, &emulator.config)
                    .with_min_lt(params.block_lt)
                    .begin_tick_tock_ext(&address, ty, &account, Some(&mut inspector))
            }
        };

        let res = 'res: {
            let output = match output {
                Ok(uncommitted) => uncommitted
                    .commit()
                    .context("Failed to commit transaction")?,
                Err(tycho_executor::TxError::Skipped) if is_external => {
                    break 'res TxEmulatorResponse::NotAccepted(TxEmulatorMsgNotAcceptedResponse {
                        success: JsonBool,
                        error: "External message not accepted by smart contract",
                        external_not_accepted: JsonBool,
                        vm_log,
                        vm_exit_code: inspector.exit_code.unwrap_or(0),
                        debug_log,
                    });
                }
                Err(e) => anyhow::bail!("Fatal executor error: {e:?}"),
            };

            TxEmulatorResponse::Success(TxEmulatorSuccessResponse {
                success: JsonBool,
                transaction: output.transaction.into_inner(),
                shard_account: output.new_state,
                vm_log,
                actions: inspector.actions,
                debug_log,
            })
        };

        let res = serde_json::to_string(&OkResponse { output: res }).unwrap();
        Ok::<_, anyhow::Error>(JsValue::from(res).unchecked_into())
    })()
    .unwrap_or_else(|e| {
        let res = serde_json::to_string(&OkResponse {
            output: TxEmulatorResponse::Error(TxEmulatorErrorResponse {
                success: JsonBool,
                error: e.to_string(),
                external_not_accepted: JsonBool,
                debug_log: String::new(),
            }),
        })
        .unwrap();
        JsValue::from(res).unchecked_into()
    })
}

#[wasm_bindgen]
pub fn run_get_method(params: &str, stack: &str, config: &str) -> js_sys::JsString {
    (|| {
        let params: RunGetMethodParams =
            serde_json::from_str(params).context("Can't decode params")?;

        let stack = Boc::decode_base64(stack).context("Failed to deserialize stack cell")?;
        let stack = stack
            .parse::<Stack>()
            .context("Failed to deserialize stack")?;

        let config = Boc::decode_base64(config).context("Failed to deserialize config cell")?;
        let config = tvm_emulator::ParsedConfig::try_from_root(config)
            .context("Failed to deserialize config")?;

        let prev_blocks = parse_prev_blocks_info(params.prev_blocks_info.as_ref())?;

        let mut emulator = TvmEmulator::new(params.code, params.data, params.verbosity);

        let subscriber = emulator.make_logger();
        let vm_log = subscriber.state().clone();
        let _tracing = tracing::subscriber::set_default(subscriber);

        emulator.args.libraries = params.libs.map(emulator_libs_to_simple).transpose()?;
        emulator.args.address = Some(params.address);
        emulator.args.now = Some(params.unixtime);
        emulator.args.balance = params.balance;
        emulator.args.extra = params.extra_currencies;
        emulator.args.rand_seed = Some(params.rand_seed);
        emulator.args.debug_enabled = params.debug_enabled;
        emulator.args.config = Some(config);
        emulator.args.prev_blocks_info = prev_blocks;

        if params.gas_limit > 0 {
            emulator.set_gas_limit(params.gas_limit);
        }

        let res = emulator.run_get_method(params.method_id, stack);
        let res = serde_json::to_string(&OkResponse {
            output: RunGetMethodResponse {
                success: JsonBool,
                stack: res.stack,
                gas_used: res.gas_used,
                debug_log: res.debug_log,
                vm_exit_code: res.exit_code,
                vm_log,
                missing_library: res.missing_library,
            },
        })
        .unwrap();
        Ok::<_, anyhow::Error>(JsValue::from(res).unchecked_into())
    })()
    .unwrap_or_else(|e| {
        let value = serde_json::to_string(&ErrResponse {
            message: e.to_string(),
        })
        .unwrap();

        JsValue::from(value).unchecked_into()
    })
}

fn emulator_libs_to_shard(libs_root: Cell) -> Result<Dict<HashBytes, LibDescr>> {
    thread_local! {
        static COMMON_PUBLISHER: Dict<HashBytes, ()> = {
            let mut dict = Dict::new();
            dict.set(HashBytes::ZERO, ()).unwrap();
            dict
        };
    }

    COMMON_PUBLISHER.with(|publishers| {
        let libs = Dict::<HashBytes, Cell>::from_raw(Some(libs_root));

        let mut items = Vec::new();
        for item in libs.iter() {
            let (hash, lib) = item.context("Invalid libraries dict")?;
            items.push((hash, LibDescr {
                lib,
                publishers: publishers.clone(),
            }));
        }

        Dict::try_from_sorted_slice(&items).context("Failed to repack libraries dict")
    })
}

fn emulator_libs_to_simple(libs_root: Cell) -> Result<Dict<HashBytes, SimpleLib>> {
    let libs = Dict::<HashBytes, Cell>::from_raw(Some(libs_root));

    let mut items = Vec::new();
    for item in libs.iter() {
        let (hash, root) = item.context("Invalid libraries dict")?;
        items.push((hash, SimpleLib { root, public: true }));
    }

    Dict::try_from_sorted_slice(&items).context("Failed to repack libraries dict")
}

fn parse_prev_blocks_info(
    prev_blocks_info: Option<&Cell>,
) -> Result<Option<tycho_vm::SafeRc<tycho_vm::Tuple>>> {
    Ok(if let Some(prev_blocks) = prev_blocks_info {
        let info_value = Stack::load_stack_value_from_cell(prev_blocks.as_ref())
            .context("Failed to deserialize previous blocks tuple")?;

        if info_value.is_null() {
            None
        } else if let Ok(tuple) = info_value.into_tuple() {
            Some(tuple)
        } else {
            anyhow::bail!("Failed to set previous blocks tuple: not a tuple");
        }
    } else {
        None
    })
}
