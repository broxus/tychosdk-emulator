use everscale_types::models::{ExtraCurrencyCollection, ShardAccount, StdAddr};
use everscale_types::prelude::*;
use serde::{Deserialize, Serialize};
use tycho_vm::{SafeRc, Stack};
use wasm_bindgen::prelude::*;

use crate::util::{serde_extra_currencies, serde_string, serde_ton_address, JsonBool};

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r###"
export type VersionInfo = {
  emulatorLibCommitHash: string;
  emulatorLibCommitDate: string;
};

export type EmulatorParams = {
  unixtime: number;
  lt: string;
  rand_seed?: string;
  ignore_chksig: boolean;
  debug_enabled: boolean;
  is_tick_tock?: boolean;
  is_tock?: boolean;
};

export type RunGetMethodParams = {
  code: string;
  data: string;
  verbosity: number;
  libs?: string;
  prev_blocks_info?: string;
  address: string;
  unixtime: number;
  balance: string;
  extra_currencies?: { [k: string]: string };
  rand_seed: string;
  gas_limit: string;
  method_id: number;
  debug_enabled: boolean;
};

export type OkResponse<T> = {
    ok: true;
    output: T;
    logs: string;
};

export type ErrResponse = {
    ok: false;
    message: string;
};

export type RunGetMethodResponse = {
    success: true;
    stack: string;
    gas_used: string;
    debug_log: string;
    vm_exit_code: number;
    vm_log: string;
    missing_library: string | null;
};

export type EmulatorResponse = EmulatorSuccess | EmulatorError;

export type EmulatorSuccess = {
  success: true;
  transaction: string;
  shard_account: string;
  debug_log: string;
  vm_log: string;
  actions: string | null;
};

export type EmulatorError = {
  success: false;
  error: string;
  debug_log: string;
} & (
  | {
      vm_log: string;
      vm_exit_code: number;
    }
  | {}
);
"###;

// === Requests ===

#[derive(Deserialize)]
pub struct EmulatorParams {
    pub unixtime: u32,
    #[serde(with = "serde_string")]
    pub lt: u64,
    #[serde(default)]
    pub rand_seed: Option<HashBytes>,
    pub ignore_chksig: bool,
    pub debug_enabled: bool,
    #[serde(default)]
    pub is_tick_tock: bool,
    #[serde(default)]
    pub is_tock: bool,
}

#[derive(Deserialize)]
pub struct RunGetMethodParams {
    #[serde(with = "Boc")]
    pub code: Cell,
    #[serde(with = "Boc")]
    pub data: Cell,
    pub verbosity: i32,
    #[serde(default, with = "Boc")]
    pub libs: Option<Cell>,
    #[serde(default, with = "Boc")]
    pub prev_blocks_info: Option<Cell>,
    #[serde(with = "serde_ton_address")]
    pub address: StdAddr,
    pub unixtime: u32,
    #[serde(with = "serde_string")]
    pub balance: u64,
    #[serde(default, with = "serde_extra_currencies")]
    pub extra_currencies: ExtraCurrencyCollection,
    pub rand_seed: HashBytes,
    #[serde(with = "serde_string")]
    pub gas_limit: u64,
    pub method_id: i32,
    pub debug_enabled: bool,
}

// === Responses ===

#[repr(transparent)]
pub struct OkResponse<T> {
    pub output: T,
}

impl<T: Serialize> Serialize for OkResponse<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut s = s.serialize_struct("OkResponse", 3)?;
        s.serialize_field("ok", &true)?;
        s.serialize_field("output", &self.output)?;
        s.serialize_field("logs", &"")?;
        s.end()
    }
}

pub struct ErrResponse<T> {
    pub message: T,
}

impl<T: std::fmt::Display> Serialize for ErrResponse<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        struct Message<'a, T>(&'a T);

        impl<T: std::fmt::Display> Serialize for Message<'_, T> {
            #[inline]
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self.0)
            }
        }

        let mut s = s.serialize_struct("ErrResponse", 2)?;
        s.serialize_field("ok", &false)?;
        s.serialize_field("message", &Message(&self.message))?;
        s.end()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RunGetMethodResponse {
    pub success: JsonBool<true>,
    #[serde(with = "BocRepr")]
    pub stack: SafeRc<Stack>,
    #[serde(with = "serde_string")]
    pub gas_used: u64,
    pub debug_log: String,
    pub vm_exit_code: i32,
    pub vm_log: String,
    pub missing_library: Option<HashBytes>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TxEmulatorResponse {
    Success(TxEmulatorSuccessResponse),
    Error(TxEmulatorErrorResponse),
    NotAccepted(TxEmulatorMsgNotAcceptedResponse),
}

#[derive(Debug, Clone, Serialize)]
pub struct TxEmulatorSuccessResponse {
    pub success: JsonBool<true>,
    #[serde(with = "Boc")]
    pub transaction: Cell,
    #[serde(with = "BocRepr")]
    pub shard_account: ShardAccount,
    pub debug_log: String,
    pub vm_log: String,
    #[serde(with = "Boc")]
    pub actions: Option<Cell>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TxEmulatorErrorResponse {
    pub success: JsonBool<false>,
    pub error: String,
    pub external_not_accepted: JsonBool<true>,
    pub debug_log: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TxEmulatorMsgNotAcceptedResponse {
    pub success: JsonBool<false>,
    pub error: &'static str,
    pub external_not_accepted: JsonBool<true>,
    pub debug_log: String,
    pub vm_log: String,
    pub vm_exit_code: i32,
}
