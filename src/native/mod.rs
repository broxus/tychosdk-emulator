use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use everscale_types::models::StdAddr;
use everscale_types::prelude::*;
use tycho_vm::{Stack, Tuple, TupleExt};

use self::models::{
    TvmEmulatorErrorResponse, TvmEmulatorRunGetMethodResponse, TvmEmulatorSendMessageResponse,
};
use crate::tvm_emulator::{ParsedConfig, TvmEmulator};
use crate::util::JsonBool;
use crate::VersionInfo;

mod models;

// === Common State ===

#[no_mangle]
pub unsafe extern "C" fn emulator_version() -> *mut c_char {
    static RESPONSE: OnceLock<String> = OnceLock::new();
    make_c_str(RESPONSE.get_or_init(|| serde_json::to_string(VersionInfo::current()).unwrap()))
}

#[no_mangle]
pub unsafe extern "C" fn emulator_set_verbosity_level(verbosity_level: c_int) -> bool {
    let level = match verbosity_level {
        0 => log::LevelFilter::Off,
        1 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        5 => log::LevelFilter::Trace,
        _ => return false,
    };
    log::set_max_level(level);
    true
}

#[no_mangle]
pub unsafe extern "C" fn emulator_config_create(config_params_boc: *const c_char) -> *mut c_void {
    ffi_new::<ParsedConfig, _>(|| parse_config(config_params_boc).map(Box::new))
}

#[no_mangle]
pub unsafe extern "C" fn emulator_config_destroy(config: *mut c_void) {
    ffi_drop::<ParsedConfig>(config)
}

// === Transaction Emulator ===

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_create(
    config_params_boc: *const c_char,
    vm_log_verbosity: c_int,
) -> *mut c_void {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn transaction_emulator_destroy(transaction_emulator: *mut c_void) {
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

// === TVM Emulator ===

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_create(
    code_boc: *const c_char,
    data_boc: *const c_char,
    _vm_log_verbosity: c_int,
) -> *mut c_void {
    ffi_new::<TvmEmulator, _>(|| {
        let code = parse_boc(code_boc).context("Failed to deserialize code boc")?;
        let data = parse_boc(data_boc).context("Failed to deserialize data boc")?;
        Ok(Box::new(TvmEmulator::new(code, data)))
    })
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_destroy(tvm_emulator: *mut c_void) {
    ffi_drop::<TvmEmulator>(tvm_emulator);
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_set_libraries(
    tvm_emulator: *mut c_void,
    libs_boc: *const c_char,
) -> bool {
    ffi_run(|| {
        let libs_dict_root = parse_boc(libs_boc).context("Failed to deserialize libraries boc")?;
        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;
        emulator.args.libraries = Some(Dict::from_raw(Some(libs_dict_root)));
        Ok(())
    })
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
    ffi_run(|| {
        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;

        let address = parse_std_addr(address).context("Failed to parse address")?;
        let config = if config.is_null() {
            None
        } else {
            parse_config(config).map(Some)?
        };
        let rand_seed = parse_hash(rand_seed_hex).context("Failed to parse rand seed")?;

        emulator.set_c7(address, unixtime, balance, &rand_seed, config);
        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_set_config_object(
    tvm_emulator: *mut c_void,
    config: *mut c_void,
) -> bool {
    ffi_run(|| {
        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;
        let config = ffi_cast::<ParsedConfig>(config)?;
        emulator.args.config = Some(config.clone());
        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_set_prev_blocks_info(
    tvm_emulator: *mut c_void,
    info_boc: *const c_char,
) -> bool {
    ffi_run(|| {
        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;
        if info_boc.is_null() {
            return Ok(());
        }

        let info_cell = parse_boc(info_boc).context("Failed to deserialize previous blocks boc")?;
        let info_value = Stack::load_stack_value_from_cell(info_cell.as_ref())
            .context("Failed to deserialize previous blocks tuple")?;

        if info_value.is_null() {
            emulator.args.prev_blocks_info = None;
        } else if let Ok(tuple) = info_value.into_tuple() {
            emulator.args.prev_blocks_info = Some(tuple);
        } else {
            anyhow::bail!("Failed to set previous blocks tuple: not a tuple");
        }
        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_set_gas_limit(
    tvm_emulator: *mut c_void,
    gas_limit: i64,
) -> bool {
    ffi_run(|| {
        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;
        emulator.set_gas_limit(gas_limit);
        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_set_debug_enabled(
    tvm_emulator: *mut c_void,
    debug_enabled: bool,
) -> bool {
    ffi_run(|| {
        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;
        emulator.args.debug_enabled = debug_enabled;
        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_run_get_method(
    tvm_emulator: *mut c_void,
    method_id: c_int,
    stack_boc: *const c_char,
) -> *mut c_char {
    ffi_run_with_response::<TvmEmulatorRunGetMethodResponse, _>(|| {
        let stack_cell = parse_boc(stack_boc).context("Failed to deserialize stack cell")?;
        let stack = stack_cell
            .parse::<Stack>()
            .context("Failed to deserialize stack")?;

        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;
        let res = emulator.run_get_method(method_id, stack);

        Ok(TvmEmulatorRunGetMethodResponse {
            success: JsonBool,
            stack: res.stack,
            gas_used: res.gas_used,
            vm_exit_code: res.exit_code,
            vm_log: res.vm_log,
            // TODO: Track libraries access in VmState.
            missing_library: None,
        })
    })
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_emulate_run_method(
    len: u32,
    params_boc: *const c_char,
    gas_limit: i64,
) -> *mut c_char {
    ffi_run_with_boc(|| {
        let params_boc = std::slice::from_raw_parts(params_boc.cast::<u8>(), len as _);
        let params_cell = Boc::decode(params_boc)?;

        let mut cs = params_cell.as_slice_allow_pruned();
        let code = cs.load_reference_cloned()?;
        let data = cs.load_reference_cloned()?;

        let mut stack_cs = cs.load_reference_as_slice()?;

        let mut params = cs.load_reference_as_slice()?;
        let mut c7_cs = params.load_reference_as_slice()?;
        let libs = if params.has_remaining(0, 1) {
            Some(params.load_reference_cloned()?)
        } else {
            None
        };

        let method_id = cs.load_u32()? as i32;

        let stack = Stack::load_from(&mut stack_cs)?;
        let c7 = Stack::load_from(&mut c7_cs)?;

        let res = {
            let mut emulator = TvmEmulator::new(code, data);
            emulator.set_gas_limit(gas_limit);
            emulator.args.raw_c7 = Some(c7.items.try_get_owned::<Tuple>(0)?);
            if libs.is_some() {
                emulator.args.libraries = Some(Dict::from_raw(libs));
            }
            emulator.run_get_method(method_id, stack)
        };

        CellBuilder::build_from((
            res.exit_code as u32,
            res.gas_used,
            CellBuilder::build_from(res.stack)?,
        ))
        .map_err(Into::into)
    })
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_send_external_message(
    tvm_emulator: *mut c_void,
    message_body_boc: *const c_char,
) -> *mut c_char {
    ffi_run_with_response::<TvmEmulatorSendMessageResponse, _>(|| {
        let message_body_cell =
            parse_boc(message_body_boc).context("Failed to parse message body boc")?;

        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;
        let res = emulator.send_external_message(message_body_cell);

        Ok(TvmEmulatorSendMessageResponse {
            success: JsonBool,
            gas_used: res.gas_used,
            vm_exit_code: res.exit_code,
            accepted: res.accepted,
            vm_log: res.vm_log,
            // TODO: Track libraries access in VmState.
            missing_library: None,
            actions: res.actions,
            new_code: res.code,
            new_data: res.data,
        })
    })
}

#[no_mangle]
pub unsafe extern "C" fn tvm_emulator_send_internal_message(
    tvm_emulator: *mut c_void,
    message_body_boc: *const c_char,
    amount: u64,
) -> *mut c_char {
    ffi_run_with_response::<TvmEmulatorSendMessageResponse, _>(|| {
        let message_body_cell =
            parse_boc(message_body_boc).context("Failed to parse message body boc")?;

        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;
        let res = emulator.send_internal_message(message_body_cell, amount);

        Ok(TvmEmulatorSendMessageResponse {
            success: JsonBool,
            gas_used: res.gas_used,
            vm_exit_code: res.exit_code,
            accepted: res.accepted,
            vm_log: res.vm_log,
            // TODO: Track libraries access in VmState.
            missing_library: None,
            actions: res.actions,
            new_code: res.code,
            new_data: res.data,
        })
    })
}

// === Utils ===

#[inline]
unsafe fn ffi_new<T, F>(f: F) -> *mut c_void
where
    F: FnOnce() -> Result<Box<T>>,
{
    match f() {
        Ok(res) => Box::into_raw(res).cast(),
        Err(e) => {
            log::error!("{e:?}");
            std::ptr::null_mut()
        }
    }
}

#[inline]
unsafe fn ffi_run<F>(f: F) -> bool
where
    F: FnOnce() -> Result<()>,
{
    match f() {
        Ok(()) => true,
        Err(e) => {
            log::error!("{e:?}");
            false
        }
    }
}

unsafe fn ffi_run_with_response<T, F>(f: F) -> *mut c_char
where
    F: FnOnce() -> Result<T>,
    T: serde::Serialize,
{
    let response = 'res: {
        let error = match f() {
            Ok(res) => match serde_json::to_string(&res) {
                Ok(res) => break 'res res,
                Err(e) => format!("Failed to serialize response: {e}"),
            },
            Err(e) => e.to_string(),
        };
        serde_json::to_string(&TvmEmulatorErrorResponse { error: &error }).unwrap()
    };

    make_c_str(&response)
}

unsafe fn ffi_run_with_boc<F>(f: F) -> *mut c_char
where
    F: FnOnce() -> Result<Cell>,
{
    match f() {
        Ok(cell) => {
            let boc = Boc::encode(cell);
            let Ok(boc_len) = u32::try_from(boc.len()) else {
                // TODO: Print error?
                return std::ptr::null_mut();
            };

            // SAFETY: `boc_len` is in `isize::MAX` bounds.
            let res = unsafe { libc::malloc(4 + boc_len as usize) };
            if !res.is_null() {
                return res.cast();
            }

            // SAFETY: `res` is not null and the allocated range is enough.
            unsafe {
                std::ptr::copy_nonoverlapping(boc_len.to_le_bytes().as_ptr(), res.cast::<u8>(), 4);
                std::ptr::copy_nonoverlapping(boc.as_ptr(), res.cast::<u8>().add(4), boc.len());
            }

            res.cast()
        }
        Err(_e) => {
            // TODO: Print error?
            std::ptr::null_mut()
        }
    }
}

unsafe fn ffi_drop<T>(value: *mut c_void) {
    _ = Box::<T>::from_raw(value.cast());
}

unsafe fn ffi_cast_mut<'a, T>(value: *mut c_void) -> Result<&'a mut T> {
    value.cast::<T>().as_mut().context("Object pointer is null")
}

unsafe fn ffi_cast<'a, T>(value: *mut c_void) -> Result<&'a T> {
    value.cast::<T>().as_ref().context("Object pointer is null")
}

unsafe fn parse_boc(boc_str: *const c_char) -> Result<Cell> {
    let boc_str = CStr::from_ptr(boc_str).to_str()?;
    Boc::decode_base64(boc_str).map_err(Into::into)
}

unsafe fn parse_config(boc_str: *const c_char) -> Result<ParsedConfig> {
    parse_boc(boc_str).and_then(ParsedConfig::try_from_root)
}

unsafe fn parse_std_addr(addr_str: *const c_char) -> Result<StdAddr> {
    let addr_str = CStr::from_ptr(addr_str).to_str()?;
    addr_str.parse::<StdAddr>().map_err(Into::into)
}

unsafe fn parse_hash(hash_str: *const c_char) -> Result<HashBytes> {
    let hash_str = CStr::from_ptr(hash_str).to_str()?;
    hash_str.parse::<HashBytes>().map_err(Into::into)
}

/// Allocates a new C-string with `malloc`.
fn make_c_str(str: &str) -> *mut c_char {
    // SAFETY: `str` is not null and its len is in `isize::MAX` bounds.
    unsafe { libc::strndup(str.as_ptr().cast(), str.len()) }
}
