use everscale_types::prelude::*;
use serde::Serialize;
use tycho_vm::{SafeRc, Stack};

use crate::util::{serde_string, JsonBool};

#[derive(Debug, Clone, Serialize)]
pub struct TvmEmulatorRunGetMethodResponse {
    pub success: JsonBool<true>,
    #[serde(with = "BocRepr")]
    pub stack: SafeRc<Stack>,
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
