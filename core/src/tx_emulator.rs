use anyhow::{Context, Result};
use everscale_types::cell::HashBytes;
use everscale_types::models::{
    BlockchainConfig, BlockchainConfigParams, ConfigParam0, GlobalCapability,
};

pub struct TxEmulator {
    pub config: tycho_executor::ParsedConfig,
    pub rand_seed: HashBytes,
    pub vm_modifiers: tycho_vm::BehaviourModifiers,
}

impl TxEmulator {
    pub fn new(params: BlockchainConfigParams) -> Result<Self> {
        anyhow::ensure!(
            params.as_dict().contains_key(19)?,
            "Global id is mandatory in config (param 19)"
        );
        anyhow::ensure!(
            params.as_dict().contains_key(43)?,
            "Size limits are mandatory in config (param 43)"
        );

        let address = params
            .get::<ConfigParam0>()?
            .context("Config account address is mandatory in config (param 0)")?;

        let config = tycho_executor::ParsedConfig::parse(BlockchainConfig { address, params }, 0)
            .context("Failed to unpack config params")?;

        let signature_with_id = config
            .global
            .capabilities
            .contains(GlobalCapability::CapSignatureWithId)
            .then_some(config.global_id);

        Ok(Self {
            config,
            rand_seed: HashBytes::ZERO,
            vm_modifiers: tycho_vm::BehaviourModifiers {
                stop_on_accept: false,
                chksig_always_succeed: false,
                signature_with_id,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use everscale_types::prelude::*;

    use super::*;

    #[test]
    fn parse_ton_config() {
        let root = Boc::decode(include_bytes!("../res/tycho_config.boc")).unwrap();
        let params = BlockchainConfigParams::from_raw(root);
        TxEmulator::new(params).unwrap();
    }
}
