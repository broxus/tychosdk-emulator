use std::borrow::Cow;
use std::str::FromStr;

use anyhow::{Context, Result};
use everscale_types::models::{BlockchainConfigParams, SizeLimitsConfig};
use everscale_types::prelude::*;
use serde::de::Error;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub emulator_lib_commit_hash: &'static str,
    pub emulator_lib_commit_date: &'static str,
}

impl VersionInfo {
    pub fn current() -> &'static Self {
        static CURRENT: VersionInfo = VersionInfo {
            emulator_lib_commit_hash: EMULATOR_COMMIT_HASH,
            emulator_lib_commit_date: EMULATOR_COMMIT_DATE,
        };

        &CURRENT
    }
}

static EMULATOR_COMMIT_HASH: &str = env!("EMULATOR_COMMIT_HASH");
static EMULATOR_COMMIT_DATE: &str = env!("EMULATOR_COMMIT_DATE");

#[derive(Clone)]
pub struct ParsedConfig {
    pub params: BlockchainConfigParams,
    // TODO: Replace with VM version.
    pub version: u32,
}

impl ParsedConfig {
    pub fn try_from_root(root: Cell) -> Result<Self> {
        let mut params = BlockchainConfigParams::from_raw(root);
        if !params.contains_raw(43)? {
            params.set_size_limits(&DEFAULT_SIZE_LIMITS)?;
        }

        // Try to unpack config to return error early.
        tycho_vm::SmcInfoTonV6::unpack_config(&params, 0)
            .context("Failed to unpack config params")?;

        let global = params
            .get_global_version()
            .context("Failed to get global version")?;

        Ok(Self {
            params,
            version: global.version,
        })
    }
}

static DEFAULT_SIZE_LIMITS: SizeLimitsConfig = SizeLimitsConfig {
    max_msg_bits: 2097152,
    max_msg_cells: 8192,
    max_library_cells: 1000,
    max_vm_data_depth: 512,
    max_ext_msg_size: 65535,
    max_ext_msg_depth: 512,
    max_acc_state_cells: 65536,
    max_acc_state_bits: 67043328,
    max_acc_public_libraries: 256,
    defer_out_queue_size_limit: 256,
};

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub struct JsonBool<const VALUE: bool>;

impl<const VALUE: bool> serde::Serialize for JsonBool<VALUE> {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bool(VALUE)
    }
}

#[allow(unused)]
pub mod serde_extra_currencies {
    use everscale_types::models::ExtraCurrencyCollection;
    use everscale_types::num::VarUint248;

    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ExtraCurrencyCollection, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        // TODO: Use unsigned here?
        #[derive(Eq, PartialEq, Hash, Deserialize)]
        #[repr(transparent)]
        struct CurrencyId(#[serde(with = "serde_value_or_string")] i32);

        #[derive(Deserialize)]
        struct Value(std::collections::HashMap<CurrencyId, VarUint248, ahash::RandomState>);

        let Some(Value(value)) = Option::<Value>::deserialize(deserializer)? else {
            return Ok(ExtraCurrencyCollection::new());
        };

        let items = value
            .into_iter()
            .map(|(CurrencyId(id), value)| (id as u32, value))
            .collect::<std::collections::BTreeMap<_, _>>();

        ExtraCurrencyCollection::try_from(items).map_err(Error::custom)
    }
}

#[allow(unused)]
pub mod serde_value_or_string {
    use serde::Deserialize;

    use super::*;

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: Deserialize<'de> + FromStr<Err: std::fmt::Display>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Value<T: FromStr<Err: std::fmt::Display>> {
            Int(T),
            String(#[serde(with = "serde_string")] T),
        }

        Ok(match Value::deserialize(deserializer)? {
            Value::Int(x) | Value::String(x) => x,
        })
    }
}

#[allow(unused)]
pub mod serde_ton_address {
    use everscale_types::models::{StdAddr, StdAddrBase64Repr};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<StdAddr, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        StdAddrBase64Repr::<true>::deserialize(deserializer)
    }

    pub fn serialize<S>(addr: &StdAddr, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        StdAddrBase64Repr::<true>::serialize(addr, serializer)
    }
}

#[allow(unused)]
pub mod serde_string {
    use super::*;

    pub fn serialize<S>(value: &dyn std::fmt::Display, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: FromStr,
        T::Err: std::fmt::Display,
    {
        BorrowedStr::deserialize(deserializer)
            .and_then(|data| T::from_str(&data.0).map_err(D::Error::custom))
    }
}

#[derive(Deserialize)]
#[repr(transparent)]
pub struct BorrowedStr<'a>(#[serde(borrow)] pub Cow<'a, str>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ton_config() {
        let root = Boc::decode(include_bytes!("../res/ton_config.boc")).unwrap();
        let config = ParsedConfig::try_from_root(root).unwrap();

        for item in config.params.as_dict().keys() {
            item.unwrap();
        }
    }
}
