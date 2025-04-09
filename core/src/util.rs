use std::borrow::Cow;
use std::str::FromStr;

use anyhow::{Context, Result};
use everscale_types::models::{BlockchainConfigParams, GlobalCapability};
use everscale_types::prelude::*;
use serde::de::Error;
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
pub fn now_sec_u64() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

#[cfg(not(all(target_arch = "wasm32")))]
pub fn now_sec_u64() -> u64 {
    use std::time::SystemTime;

    (SystemTime::now().duration_since(SystemTime::UNIX_EPOCH))
        .unwrap()
        .as_secs()
}

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

        let signature_with_id = if global
            .capabilities
            .contains(GlobalCapability::CapSignatureWithId)
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
            signature_with_id,
        })
    }
}

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
        struct CurrencyId(#[serde(with = "serde_string")] i32);

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
        let root = Boc::decode(include_bytes!("../res/tycho_config.boc")).unwrap();
        let config = ParsedConfig::try_from_root(root).unwrap();

        for item in config.params.as_dict().keys() {
            item.unwrap();
        }
    }
}
