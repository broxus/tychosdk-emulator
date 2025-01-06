#[cfg(feature = "native")]
pub mod native;

mod models;
mod tvm_emulator;
mod util;

static EMULATOR_VERSION: &str = env!("TYCHO_EMULATOR_VERSION");
static EMULATOR_BUILD: &str = env!("TYCHO_EMULATOR_BUILD");
