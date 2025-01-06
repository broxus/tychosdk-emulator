use std::rc::Rc;

use everscale_types::prelude::*;
use everscale_vm::Stack;
use serde::Serialize;

use crate::util::{serde_string, JsonBool};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionResponse {
    pub emulator_lib_version: &'static str,
    pub emulator_lib_build: &'static str,
}

impl VersionResponse {
    pub const CURRENT: Self = Self {
        emulator_lib_version: crate::EMULATOR_VERSION,
        emulator_lib_build: crate::EMULATOR_BUILD,
    };
}

#[derive(Debug, Clone, Serialize)]
pub struct TvmEmulatorRunGetMethodResponse {
    pub success: JsonBool<true>,
    #[serde(with = "BocRepr")]
    pub stack: Rc<Stack>,
    #[serde(with = "serde_string")]
    pub gas_used: u64,
    pub vm_exit_code: i32,
    pub vm_log: String,
    pub missing_library: Option<HashBytes>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TvmEmulatorSendMessageResponse {
    pub success: JsonBool<true>,
    #[serde(with = "serde_string")]
    pub gas_used: u64,
    pub vm_exit_code: i32,
    pub accepted: bool,
    pub vm_log: String,
    pub missing_library: Option<HashBytes>,
    #[serde(with = "Boc")]
    pub actions: Option<Cell>,
    #[serde(with = "Boc")]
    pub new_code: Cell,
    #[serde(with = "Boc")]
    pub new_data: Cell,
}

#[derive(Debug, Clone, Copy)]
pub struct TvmEmulatorErrorResponse<'a> {
    pub error: &'a str,
}

impl Serialize for TvmEmulatorErrorResponse<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut s = serializer.serialize_struct("TvmEmulatorErrorResponse", 3)?;
        s.serialize_field("success", &false)?;
        s.serialize_field("error", self.error)?;
        s.serialize_field("external_not_accepted", &false)?;
        s.end()
    }
}
