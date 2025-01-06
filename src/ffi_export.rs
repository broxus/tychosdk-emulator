use std::ffi::{c_char, c_int, c_void, CStr};

use anyhow::Result;
use everscale_types::prelude::*;

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_create(
    config_params_boc: *const c_char,
    vm_log_verbosity: c_int,
) -> *mut c_void {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn emulator_config_create(config_params_boc: *const c_char) -> *mut c_void {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_set_unixtime(
    transaction_emulator: *mut c_void,
    unixtime: u32,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_set_lt(
    transaction_emulator: *mut c_void,
    lt: u64,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_set_rand_seed(
    transaction_emulator: *mut c_void,
    rand_seed_hex: *const c_char,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_set_ignore_chksig(
    transaction_emulator: *mut c_void,
    ignore_chksig: bool,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_set_config(
    transaction_emulator: *mut c_void,
    config_boc: *const c_char,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_set_config_object(
    transaction_emulator: *mut c_void,
    config: *mut c_void,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_set_libs(
    transaction_emulator: *mut c_void,
    libs_boc: *const c_char,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_set_debug_enabled(
    transaction_emulator: *mut c_void,
    debug_enabled: bool,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_set_prev_blocks_info(
    transaction_emulator: *mut c_void,
    info_boc: *const c_char,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_emulate_transaction(
    transaction_emulator: *mut c_void,
    shard_account_boc: *const c_char,
    message_boc: *const c_char,
) -> *mut c_char {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_emulate_tick_tock_transaction(
    transaction_emulator: *mut c_void,
    shard_account_boc: *const c_char,
    is_tock: bool,
) -> *mut c_char {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_destroy(transaction_emulator: *mut c_void) {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn emulator_set_verbosity_level(verbosity_level: c_int) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_create(
    code_boc: *const c_char,
    data_boc: *const c_char,
    vm_log_verbosity: c_int,
) -> *mut c_void {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_set_libraries(
    tvm_emulator: *mut c_void,
    libc_boc: *const c_char,
) -> *mut c_void {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_set_c7(
    tvm_emulator: *mut c_void,
    address: *const c_char,
    unixtime: u32,
    balance: u64,
    rand_seed_hex: *const c_char,
    config: *const c_char,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_set_config_object(
    tvm_emulator: *mut c_void,
    config: *mut c_void,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_set_prev_blocks_info(
    tvm_emulator: *mut c_void,
    info_boc: *const c_char,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_set_gas_limit(
    tvm_emulator: *mut c_void,
    gas_limit: i64,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_set_debug_enabled(
    tvm_emulator: *mut c_void,
    debug_enabled: bool,
) -> bool {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_run_get_method(
    tvm_emulator: *mut c_void,
    method_id: c_int,
    stack_boc: *const c_char,
) -> *mut c_char {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_emulate_run_method(
    len: u32,
    params_boc: *const c_char,
    gas_limit: i64,
) -> *mut c_char {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_send_external_message(
    tvm_emulator: *mut c_void,
    message_body_boc: *const c_char,
) -> *mut c_char {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_send_internal_message(
    tvm_emulator: *mut c_void,
    message_body_boc: *const c_char,
    amount: u64,
) -> *mut c_char {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_destroy(tvm_emulator: *mut c_void) {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn emulator_config_destroy(config: *mut c_void) {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn emulator_version() -> *mut c_char {
    todo!()
}

// === Utils ===

unsafe fn parse_boc(boc_str: *const c_char) -> Result<Cell> {
    let boc_str = CStr::from_ptr(boc_str).to_str()?;
    Boc::decode_base64(boc_str).map_err(Into::into)
}

unsafe fn make_c_str(str: &str) -> *mut c_char {
    libc::strndup(str.as_ptr().cast(), str.len())
}
