use std::sync::OnceLock;

use anyhow::{Context, Result};
use everscale_types::models::{BlockchainConfigParams, IntAddr, MsgInfo, ShardAccount, TickTock};
use everscale_types::prelude::*;
use tycho_vm::Stack;
use wasm_bindgen::prelude::*;

use self::models::{
    EmulatorParams, ErrResponse, OkResponse, RunGetMethodParams, RunGetMethodResponse,
    TxEmulatorErrorResponse, TxEmulatorMsgNotAcceptedResponse, TxEmulatorResponse,
    TxEmulatorSuccessResponse,
};
use self::tvm_emulator::TvmEmulator;
use self::tx_emulator::TxEmulator;
use self::util::{now_sec_u64, JsonBool, ParsedConfig, VersionInfo};

pub mod models;
pub mod tvm_emulator;
pub mod tx_emulator;
pub mod util;

// === Exported Methods ===

#[wasm_bindgen]
pub fn version() -> js_sys::JsString {
    static RESPONSE: OnceLock<String> = OnceLock::new();
    let info = RESPONSE.get_or_init(|| serde_json::to_string(VersionInfo::current()).unwrap());
    JsValue::from_str(info.as_str()).unchecked_into()
}

#[wasm_bindgen]
pub fn create_emulator(config: &str, verbosity: i32) -> Result<*mut TxEmulator, JsError> {
    (|| {
        _ = verbosity;

        let config = Boc::decode_base64(config)?;
        let emulator = TxEmulator::new(BlockchainConfigParams::from_raw(config))?;
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
    let (emulator, params, libraries) = match (|| {
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
            Dict::from_raw(Some(root))
        } else {
            Dict::new()
        };

        Ok((emulator, params, libraries))
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
            vm_modifiers: tycho_vm::BehaviourModifiers {
                chksig_always_succeed: params.ignore_chksig,
                ..emulator.vm_modifiers
            },
            disable_delete_frozen_accounts: true,
            charge_action_fees_on_fail: true,
            full_body_in_bounced: true,
        };

        let mut debug_log = String::new();
        let mut inspector = tycho_executor::ExecutorInspector {
            debug: debug_enabled.then_some(&mut debug_log),
            ..Default::default()
        };

        let output = match message {
            Some((msg_root, _)) => tycho_executor::Executor::new(&params, &emulator.config)
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
                tycho_executor::Executor::new(&params, &emulator.config).begin_tick_tock_ext(
                    &address,
                    ty,
                    &account,
                    Some(&mut inspector),
                )
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
                        // TODO: Somehow collect the log from the compute phase.
                        vm_log: String::new(),
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
                // TODO: Somehow collect the log from the compute phase.
                vm_log: String::new(),
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
        _ = params.verbosity;

        let stack = Boc::decode_base64(stack).context("Failed to deserialize stack cell")?;
        let stack = stack
            .parse::<Stack>()
            .context("Failed to deserialize stack")?;

        let config = Boc::decode_base64(config).context("Failed to deserialize config cell")?;
        let config = ParsedConfig::try_from_root(config).context("Failed to deserialize config")?;

        let prev_blocks = if let Some(prev_blocks) = params.prev_blocks_info {
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
        };

        let mut emulator = TvmEmulator::new(params.code, params.data);
        emulator.args.libraries = Some(Dict::from_raw(params.libs));
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
                vm_log: res.vm_log,
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
