use std::borrow::Cow;
use std::str::FromStr;

use anyhow::Result;
use serde::Deserialize;
use serde::de::Error;
use tycho_vm::VmLogMask;

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

pub fn make_vm_log_mask(verbosity: i32, allow_c5: bool) -> VmLogMask {
    let mut res = VmLogMask::empty();
    if verbosity != 0 {
        res |= VmLogMask::MESSAGE;
    }

    if verbosity > 1 {
        res |= VmLogMask::EXEC_LOCATION;
        if verbosity > 2 {
            res |= VmLogMask::GAS_REMAINING;
            if verbosity > 3 {
                res |= VmLogMask::DUMP_STACK;
                if verbosity > 4 {
                    res |= VmLogMask::DUMP_STACK_VERBOSE;
                    if allow_c5 {
                        res |= VmLogMask::DUMP_C5;
                    }
                }
            }
        }
    }
    res
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
    use tycho_types::models::ExtraCurrencyCollection;
    use tycho_types::num::VarUint248;

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
    use tycho_types::models::{StdAddr, StdAddrBase64Repr};

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
