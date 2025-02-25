use everscale_types::cell::HashBytes;
use everscale_types::dict::Dict;
use everscale_types::models::LibDescr;
use tycho_executor::ExecutorParams;
use tycho_vm::{SafeRc, Tuple};

use crate::util::ParsedConfig;

pub struct TxEmulator {
    pub config: ParsedConfig,
    pub libraries: Dict<HashBytes, LibDescr>,
    pub prev_blocks_info: Option<SafeRc<Tuple>>,
    pub block_unixtime: u32,
    pub lt: u64,
    pub rand_seed: HashBytes,
    pub vm_modifiers: tycho_vm::BehaviourModifiers,
}

impl TxEmulator {
    pub fn new(config: ParsedConfig) -> Self {
        Self {
            config,
            libraries: Dict::new(),
            prev_blocks_info: None,
            block_unixtime: 0,
            lt: 0,
            rand_seed: HashBytes::ZERO,
            vm_modifiers: tycho_vm::BehaviourModifiers {
                stop_on_accept: false,
                chksig_always_succeed: false,
                signature_with_id: None,
            },
        }
    }

    pub fn make_params(&self) -> ExecutorParams {
        ExecutorParams {
            libraries: self.libraries.clone(),
            rand_seed: self.rand_seed,
            block_unixtime: self.block_unixtime,
            block_lt: self.lt,
            vm_modifiers: self.vm_modifiers,
            disable_delete_frozen_accounts: true,
            charge_action_fees_on_fail: true,
            full_body_in_bounced: true,
        }
    }
}
