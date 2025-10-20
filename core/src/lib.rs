pub mod models;
pub mod subscriber;
pub mod tvm_emulator;
pub mod tx_emulator;
pub mod util;

#[cfg(feature = "native")]
pub mod native;
#[cfg(feature = "wasm")]
pub mod wasm;

pub static EMULATOR_COMMIT_HASH: &str = env!("EMULATOR_COMMIT_HASH");
pub static EMULATOR_COMMIT_DATE: &str = env!("EMULATOR_COMMIT_DATE");
