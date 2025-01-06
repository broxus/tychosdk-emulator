#[cfg(feature = "native")]
pub mod ffi_export;

static EMULATOR_VERSION: &str = env!("TYCHO_EMULATOR_VERSION");
static EMULATOR_BUILD: &str = env!("TYCHO_EMULATOR_BUILD");
