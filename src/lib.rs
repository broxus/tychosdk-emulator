use serde::Serialize;

#[cfg(feature = "native")]
pub mod native;

mod tvm_emulator;
mod util;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub emulator_lib_version: &'static str,
    pub emulator_lib_build: &'static str,
}

impl VersionInfo {
    pub fn current() -> &'static Self {
        static CURRENT: VersionInfo = VersionInfo {
            emulator_lib_version: crate::EMULATOR_VERSION,
            emulator_lib_build: crate::EMULATOR_BUILD,
        };

        &CURRENT
    }
}

static EMULATOR_VERSION: &str = env!("TYCHO_EMULATOR_VERSION");
static EMULATOR_BUILD: &str = env!("TYCHO_EMULATOR_BUILD");
