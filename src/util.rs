use std::borrow::Cow;
use std::str::FromStr;

use anyhow::{Context, Result};
use everscale_types::models::BlockchainConfigParams;
use everscale_types::prelude::*;
use serde::de::Error;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub emulator_lib_version: &'static str,
    pub emulator_lib_build: &'static str,
}

impl VersionInfo {
    pub fn current() -> &'static Self {
        static CURRENT: VersionInfo = VersionInfo {
            emulator_lib_version: EMULATOR_VERSION,
            emulator_lib_build: EMULATOR_BUILD,
        };

        &CURRENT
    }
}

static EMULATOR_VERSION: &str = env!("TYCHO_EMULATOR_VERSION");
static EMULATOR_BUILD: &str = env!("TYCHO_EMULATOR_BUILD");

#[derive(Clone)]
pub struct ParsedConfig {
    pub params: BlockchainConfigParams,
    // TODO: Replace with VM version.
    pub version: u32,
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

        Ok(Self {
            params,
            version: global.version,
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
