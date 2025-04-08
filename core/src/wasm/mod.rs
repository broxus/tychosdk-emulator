use std::str::FromStr;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use everscale_types::models::{
    BlockchainConfig, ConfigParam0, ExtraCurrencyCollection, IntAddr, MsgInfo, ShardAccount,
    StdAddr, TickTock,
};
use everscale_types::prelude::*;
use serde::{Deserialize, Serialize};
use tycho_vm::Stack;
use wasm_bindgen::prelude::*;

use crate::models::{
    TvmEmulatorRunGetMethodResponse, TxEmulatorMsgNotAcceptedResponse, TxEmulatorResponse,
    TxEmulatorSuccessResponse,
};
use crate::tvm_emulator::TvmEmulator;
use crate::tx_emulator::TxEmulator;
use crate::util::{
    serde_extra_currencies, serde_string, serde_ton_address, serde_value_or_string, JsonBool,
    ParsedConfig, VersionInfo,
};

mod instant {
    cfg_if::cfg_if! {
        if #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))] {
            use std::time::Duration;

            #[derive(Clone, Copy)]
            pub struct Instant(Duration);

            impl Instant {
                #[inline]
                pub fn now() -> Self {
                    Self(duration_from_f64(now()))
                }

                #[inline]
                pub fn duration_since(&self, earlier: Instant) -> Duration {
                    assert!(
                        earlier.0 <= self.0,
                        "`earlier` cannot be later than `self`."
                    );
                    self.0 - earlier.0
                }

                #[inline]
                pub fn elapsed(&self) -> Duration {
                    Self::now().duration_since(*self)
                }
            }

            pub fn now() -> f64 {
                use wasm_bindgen::prelude::*;
                js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("performance"))
                    .expect("failed to get performance from global object")
                    .unchecked_into::<web_sys::Performance>()
                    .now()
            }

            fn duration_from_f64(millis: f64) -> Duration {
                Duration::from_millis(millis.trunc() as u64)
                    + Duration::from_nanos((millis.fract() * 1.0e6) as u64)
            }
        } else {
            pub use std::time::Instant;
        }
    }
}

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
        let config = ParsedConfig::try_from_root(config)?;

        Ok::<_, anyhow::Error>(Box::into_raw(Box::new(TxEmulator::new(config))))
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
    (|| {
        anyhow::ensure!(!emulator.is_null(), "transaction_emulator is null");

        let params: TransactionEmulationParams =
            serde_json::from_str(params).context("Can't decode params")?;

        anyhow::ensure!(
            params.is_tick_tock || !params.is_tock,
            "Inconsistent parameters is_tick_tock=false, is_tock=true"
        );
        let is_tock = params.is_tock;

        let libraries = if let Some(libs) = libs {
            let root = Boc::decode_base64(libs).context("Failed to deserialize libraries")?;
            Dict::from_raw(Some(root))
        } else {
            Dict::new()
        };

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
            let msg_info = msg.parse::<MsgInfo>()?;
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

        let IntAddr::Std(address) = (match account.load_account()? {
            Some(account) => account.address,
            None => match message.as_ref().map(|(_, info)| info) {
                Some(MsgInfo::Int(info)) => info.dst.clone(),
                Some(MsgInfo::ExtIn(info)) => info.dst.clone(),
                Some(MsgInfo::ExtOut(_)) => {
                    anyhow::bail!("Only internal and external inbound messages are accepted");
                }
                None => anyhow::bail!("Can't run tick-tock transaction on account_none"),
            },
        }) else {
            anyhow::bail!("var_addr is not supported");
        };

        let emulator = unsafe { &mut *emulator };
        emulator.libraries = libraries;
        emulator.block_unixtime = params.utime;
        emulator.lt = params.lt;
        // TODO: Add support for `signature_with_id`.
        emulator.vm_modifiers.chksig_always_succeed = params.ignore_chksig;

        if let Some(rand_seed) = params.rand_seed {
            emulator.rand_seed = rand_seed;
        }

        // TODO: Add support for `signature_with_id`.
        let mut params = emulator.make_params();
        if params.block_unixtime == 0 {
            params.block_unixtime = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as u32;
        }

        let config = tycho_executor::ParsedConfig::parse(
            BlockchainConfig {
                address: match emulator.config.params.get::<ConfigParam0>()? {
                    Some(address) => address,
                    None => anyhow::bail!("Can't find a config address (param 0)"),
                },
                params: emulator.config.params.clone(),
            },
            params.block_unixtime,
        )
        .context("Failed to unpack blockchain config")?;

        let since = instant::Instant::now();
        let output = match message {
            Some((msg, _)) => tycho_executor::Executor::new(&params, &config).begin_ordinary(
                &address,
                is_external,
                msg,
                &account,
            ),
            None => {
                let ty = if is_tock {
                    TickTock::Tock
                } else {
                    TickTock::Tick
                };
                tycho_executor::Executor::new(&params, &config)
                    .begin_tick_tock(&address, ty, &account)
            }
        };

        let res = (move || {
            let output = match output {
                Ok(uncommitted) => uncommitted
                    .commit()
                    .context("Failed to commit transaction")?,
                Err(tycho_executor::TxError::Skipped) if is_external => {
                    return Ok(TxEmulatorResponse::NotAccepted(
                        TxEmulatorMsgNotAcceptedResponse {
                            success: JsonBool,
                            error: "External message not accepted by smart contract",
                            external_not_accepted: JsonBool,
                            vm_log: String::new(),
                            // TODO: Somehow get exit code from the execution result.
                            vm_exit_code: 0,
                            elapsed_time: since.elapsed().as_secs_f64(),
                        },
                    ));
                }
                Err(e) => anyhow::bail!("Fatal executor error: {e:?}"),
            };

            Ok(TxEmulatorResponse::Success(TxEmulatorSuccessResponse {
                success: JsonBool,
                transaction: output.transaction.into_inner(),
                shard_account: output.new_state,
                // TODO: Somehow collect the log from the compute phase.
                vm_log: String::new(),
                // TODO: Somehow collect actions from the compute phase.
                actions: None,
                elapsed_time: since.elapsed().as_secs_f64(),
            }))
        })()?;

        let res = serde_json::to_string(&RunGetMethodOkResponse { output: res }).unwrap();
        Ok::<_, anyhow::Error>(JsValue::from(res).unchecked_into())
    })()
    .unwrap_or_else(|e| {
        let value = serde_json::to_string(&RunGetMethodErrResponse {
            message: format!("{e:?}"),
        })
        .unwrap();

        JsValue::from(value).unchecked_into()
    })
}

#[wasm_bindgen]
pub fn emulate(
    config: &str,
    libs: Option<String>,
    verbosity: i32,
    account: &str,
    message: Option<String>,
    params: &str,
) -> Result<js_sys::JsString, JsError> {
    let emulator = create_emulator(config, verbosity)?;
    let res = emulate_with_emulator(emulator, libs, account, message, params);
    destroy_emulator(emulator)?;
    Ok(res)
}

#[wasm_bindgen]
pub fn run_get_method(params: &str, stack: &str, config: Option<String>) -> js_sys::JsString {
    (|| {
        let params: GetMethodParams =
            serde_json::from_str(params).context("Can't decode params")?;
        _ = params.verbosity;

        let stack = Boc::decode_base64(stack).context("Failed to deserialize stack cell")?;
        let stack = stack
            .parse::<Stack>()
            .context("Failed to deserialize stack")?;

        let config = match config {
            Some(config) => {
                let c = Boc::decode_base64(config).context("Failed to deserialize config cell")?;
                ParsedConfig::try_from_root(c)
                    .map(Some)
                    .context("Failed to deserialize config")?
            }
            None => None,
        };

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
        emulator.args.config = config;
        emulator.args.prev_blocks_info = prev_blocks;

        if params.gas_limit > 0 {
            emulator.set_gas_limit(params.gas_limit);
        }

        let res = emulator.run_get_method(params.method_id, stack);
        let res = serde_json::to_string(&RunGetMethodOkResponse {
            output: TvmEmulatorRunGetMethodResponse {
                success: JsonBool,
                stack: res.stack,
                gas_used: res.gas_used,
                vm_exit_code: res.exit_code,
                vm_log: res.vm_log,
                missing_library: res.missing_library,
            },
        })
        .unwrap();
        Ok::<_, anyhow::Error>(JsValue::from(res).unchecked_into())
    })()
    .unwrap_or_else(|e| {
        let value = serde_json::to_string(&RunGetMethodErrResponse {
            message: e.to_string(),
        })
        .unwrap();

        JsValue::from(value).unchecked_into()
    })
}

// === Models ===

#[derive(Deserialize)]
struct TransactionEmulationParams {
    utime: u32,
    #[serde(with = "serde_string")]
    lt: u64,
    #[serde(default, with = "serde_empty_string_or_hash")]
    rand_seed: Option<HashBytes>,
    ignore_chksig: bool,
    #[allow(unused)]
    debug_enabled: bool,
    #[serde(default)]
    is_tick_tock: bool,
    #[serde(default)]
    is_tock: bool,
}

mod serde_empty_string_or_hash {
    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<HashBytes>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Deserialize)]
        struct Value<'a>(#[serde(borrow)] std::borrow::Cow<'a, str>);

        let Some(Value(value)) = Option::<Value>::deserialize(deserializer)? else {
            return Ok(None);
        };
        if value.is_empty() {
            return Ok(None);
        }

        HashBytes::from_str(value.as_ref())
            .map(Some)
            .map_err(Error::custom)
    }
}

#[derive(Deserialize)]
struct GetMethodParams {
    #[serde(with = "Boc")]
    code: Cell,
    #[serde(with = "Boc")]
    data: Cell,
    verbosity: i32,
    #[serde(default, with = "serde_empty_string_or_boc")]
    libs: Option<Cell>,
    #[serde(default, with = "serde_empty_string_or_boc")]
    prev_blocks_info: Option<Cell>,
    #[serde(with = "serde_ton_address")]
    address: StdAddr,
    unixtime: u32,
    #[serde(with = "serde_string")]
    balance: u64,
    #[serde(default, with = "serde_extra_currencies")]
    extra_currencies: ExtraCurrencyCollection,
    rand_seed: HashBytes,
    #[serde(with = "serde_value_or_string")]
    gas_limit: i64,
    method_id: i32,
    debug_enabled: bool,
}

mod serde_empty_string_or_boc {
    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Cell>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Deserialize)]
        struct Value<'a>(#[serde(borrow)] std::borrow::Cow<'a, str>);

        let Some(Value(value)) = Option::<Value>::deserialize(deserializer)? else {
            return Ok(None);
        };
        if value.is_empty() {
            return Ok(None);
        }

        Boc::decode_base64(value.as_bytes())
            .map(Some)
            .map_err(Error::custom)
    }
}

#[repr(transparent)]
struct RunGetMethodOkResponse<T> {
    output: T,
}

impl<T: Serialize> Serialize for RunGetMethodOkResponse<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut s = s.serialize_struct("RunGetMethodErrResponse", 2)?;
        s.serialize_field("output", &self.output)?;
        s.serialize_field("logs", &"")?;
        s.end()
    }
}

struct RunGetMethodErrResponse<T> {
    message: T,
}

impl<T: std::fmt::Display> Serialize for RunGetMethodErrResponse<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        struct Message<'a, T>(&'a T);

        impl<T: std::fmt::Display> Serialize for Message<'_, T> {
            #[inline]
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self.0)
            }
        }

        let mut s = s.serialize_struct("RunGetMethodErrResponse", 2)?;
        s.serialize_field("fail", &true)?;
        s.serialize_field("message", &Message(&self.message))?;
        s.end()
    }
}

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r###"
export type VersionInfo = {
  emulatorLibCommitHash: string;
  emulatorLibCommitDate: string;
};

export type GetMethodInternalParams = {
  code: string;
  data: string;
  verbosity: number;
  libs: string;
  address: string;
  unixtime: number;
  balance: string;
  rand_seed: string;
  gas_limit: string;
  method_id: number;
  debug_enabled: boolean;
  extra_currencies?: { [k: string]: string };
};

export type EmulationInternalParams = {
  utime: number;
  lt: string;
  rand_seed: string;
  ignore_chksig: boolean;
  debug_enabled: boolean;
  is_tick_tock?: boolean;
  is_tock?: boolean;
};

export type ResultSuccess = {
  success: true;
  transaction: string;
  shard_account: string;
  vm_log: string;
  actions: string | null;
};

export type ResultError = {
  success: false;
  error: string;
} & (
  | {
      vm_log: string;
      vm_exit_code: number;
    }
  | {}
);
"###;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ton_config() {
        let root = Boc::decode(include_bytes!("../../res/ton_config.boc")).unwrap();
        let config = ParsedConfig::try_from_root(root).unwrap();

        tycho_executor::ParsedConfig::parse(
            BlockchainConfig {
                address: match config.params.get::<ConfigParam0>().unwrap() {
                    Some(address) => address,
                    None => panic!("Can't find a config address (param 0)"),
                },
                params: config.params.clone(),
            },
            1744140281,
        )
        .unwrap();
    }
}
