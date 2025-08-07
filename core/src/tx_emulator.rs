use anyhow::{Context, Result};
use tycho_types::cell::HashBytes;
use tycho_types::models::{
    BlockchainConfig, BlockchainConfigParams, ConfigParam0, GlobalCapability, SizeLimitsConfig,
};

use crate::subscriber::VmLogSubscriber;
use crate::util::make_vm_log_mask;

pub struct TxEmulator {
    pub config: tycho_executor::ParsedConfig,
    pub rand_seed: HashBytes,
    pub verbosity: i32,
    pub vm_modifiers: tycho_vm::BehaviourModifiers,
}

impl TxEmulator {
    pub fn new(mut params: BlockchainConfigParams, verbosity: i32) -> Result<Self> {
        if !params.as_dict().contains_key(43)? {
            params
                .set_size_limits(&DEFAULT_SIZE_LIMITS)
                .context("Failed to set default size limits (param 43)")?;
        }

        let address = params
            .get::<ConfigParam0>()?
            .context("Config account address is mandatory in config (param 0)")?;

        let config = tycho_executor::ParsedConfig::parse(
            BlockchainConfig {
                address,
                params: params.clone(),
            },
            0,
        )
        .context("Failed to unpack config params")?;

        let signature_with_id = if config
            .global
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
            config,
            rand_seed: HashBytes::ZERO,
            verbosity,
            vm_modifiers: tycho_vm::BehaviourModifiers {
                stop_on_accept: false,
                chksig_always_succeed: false,
                signature_with_id,
                log_mask: make_vm_log_mask(verbosity, true),
            },
        })
    }

    pub fn make_logger(&self) -> VmLogSubscriber {
        let mut log_max_size = 256;
        if self.verbosity > 4 {
            log_max_size = 32 << 20;
        } else if self.verbosity > 0 {
            log_max_size = 1 << 20;
        }

        VmLogSubscriber::new(self.vm_modifiers.log_mask, log_max_size)
    }
}

static DEFAULT_SIZE_LIMITS: SizeLimitsConfig = SizeLimitsConfig {
    max_msg_bits: 1 << 21,
    max_msg_cells: 1 << 13,
    max_library_cells: 1000,
    max_vm_data_depth: 512,
    max_ext_msg_size: 65535,
    max_ext_msg_depth: 512,
    max_acc_state_cells: 1 << 16,
    max_acc_state_bits: (1 << 16) * 1023,
    max_acc_public_libraries: 256,
    defer_out_queue_size_limit: 256,
};

#[cfg(test)]
mod tests {
    use tycho_types::prelude::*;

    use super::*;

    #[test]
    fn parse_tycho_config() {
        let root = Boc::decode(include_bytes!("../res/tycho_config.boc")).unwrap();
        let params = BlockchainConfigParams::from_raw(root);
        TxEmulator::new(params, 0).unwrap();
    }

    #[test]
    fn parse_ton_config() {
        let root = Boc::decode(include_bytes!("../res/ton_config.boc")).unwrap();
        let params = BlockchainConfigParams::from_raw(root);
        TxEmulator::new(params, 0).unwrap();
    }
}
