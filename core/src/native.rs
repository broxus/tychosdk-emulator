#![allow(clippy::missing_safety_doc)]
#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{Context, Result};
use tycho_types::models::{IntAddr, LibDescr, MsgInfo, ShardAccount, StdAddr, TickTock};
use tycho_types::prelude::*;
use tycho_vm::{SafeRc, Stack, Tuple, TupleExt};

use crate::models::{
    RunGetMethodResponse, TvmEmulatorErrorResponse, TvmEmulatorSendMessageResponse,
    TxEmulatorMsgNotAcceptedResponse, TxEmulatorResponse, TxEmulatorSuccessResponse, VersionInfo,
};
use crate::tvm_emulator::{self, TvmEmulator};
use crate::tx_emulator::TxEmulator;
use crate::util::{JsonBool, now_sec_u64};

static VERBOSITY_LEVEL: AtomicU32 = AtomicU32::new(0);

// === FFI Stuff ===

#[unsafe(no_mangle)]
pub unsafe extern "C" fn string_destroy(string: *mut c_char) {
    libc::free(string.cast());
}

// === Common State ===

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emulator_version() -> *mut c_char {
    static RESPONSE: OnceLock<String> = OnceLock::new();
    make_c_str(RESPONSE.get_or_init(|| {
        serde_json::to_string(&VersionInfo {
            emulator_lib_commit_hash: crate::EMULATOR_COMMIT_HASH,
            emulator_lib_commit_date: crate::EMULATOR_COMMIT_DATE,
        })
        .unwrap()
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emulator_set_verbosity_level(verbosity_level: c_int) -> bool {
    // TODO: Port to `tracing`

    // let level = match verbosity_level {
    //     0 => log::LevelFilter::Off,
    //     1 => log::LevelFilter::Error,
    //     2 => log::LevelFilter::Warn,
    //     3 => log::LevelFilter::Info,
    //     4 => log::LevelFilter::Debug,
    //     5 => log::LevelFilter::Trace,
    //     _ => return false,
    // };
    // log::set_max_level(level);

    VERBOSITY_LEVEL.store(verbosity_level as u32, Ordering::Relaxed);

    true
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emulator_config_create(config_params_boc: *const c_char) -> *mut c_void {
    ffi_new::<tvm_emulator::ParsedConfig, _>(|| parse_config(config_params_boc).map(Box::new))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emulator_config_destroy(config: *mut c_void) {
    ffi_drop::<tvm_emulator::ParsedConfig>(config)
}

// === Transaction Emulator ===

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_create(
    config_params_boc: *const c_char,
    vm_log_verbosity: c_int,
) -> *mut c_void {
    ffi_new::<TxEmulatorExt, _>(|| {
        let config = parse_config(config_params_boc)?;
        Ok(Box::new(TxEmulatorExt {
            base: TxEmulator::new(config.params, vm_log_verbosity)?,
            block_unixtime: 0,
            lt: 0,
            libraries: Dict::new(),
            prev_blocks_info: None,
            debug_enabled: false,
        }))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_destroy(transaction_emulator: *mut c_void) {
    ffi_drop::<TxEmulatorExt>(transaction_emulator)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_set_unixtime(
    transaction_emulator: *mut c_void,
    unixtime: u32,
) -> bool {
    ffi_run(|| {
        let emulator = ffi_cast_mut::<TxEmulatorExt>(transaction_emulator)?;
        emulator.block_unixtime = unixtime;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_set_lt(
    transaction_emulator: *mut c_void,
    lt: u64,
) -> bool {
    ffi_run(|| {
        let emulator = ffi_cast_mut::<TxEmulatorExt>(transaction_emulator)?;
        emulator.lt = lt;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_set_rand_seed(
    transaction_emulator: *mut c_void,
    rand_seed_hex: *const c_char,
) -> bool {
    ffi_run(|| {
        let rand_seed = parse_hash(rand_seed_hex).context("Failed to parse rand seed")?;
        let emulator = ffi_cast_mut::<TxEmulatorExt>(transaction_emulator)?;
        emulator.base.rand_seed = rand_seed;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_set_ignore_chksig(
    transaction_emulator: *mut c_void,
    ignore_chksig: bool,
) -> bool {
    ffi_run(|| {
        let emulator = ffi_cast_mut::<TxEmulatorExt>(transaction_emulator)?;
        emulator.base.vm_modifiers.chksig_always_succeed = ignore_chksig;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_set_config(
    transaction_emulator: *mut c_void,
    config_boc: *const c_char,
) -> bool {
    ffi_run(|| {
        let config = parse_config(config_boc)?;
        let emulator = ffi_cast_mut::<TxEmulatorExt>(transaction_emulator)?;
        emulator.rebuild_executor(&config)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_set_config_object(
    transaction_emulator: *mut c_void,
    config: *mut c_void,
) -> bool {
    ffi_run(|| {
        let config = ffi_cast::<tvm_emulator::ParsedConfig>(config)?;
        let emulator = ffi_cast_mut::<TxEmulatorExt>(transaction_emulator)?;
        emulator.rebuild_executor(config)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_set_libs(
    transaction_emulator: *mut c_void,
    libs_boc: *const c_char,
) -> bool {
    ffi_run(|| {
        let emulator = ffi_cast_mut::<TxEmulatorExt>(transaction_emulator)?;

        if libs_boc.is_null() {
            // NOTE: This behaviour is different from the reference, but it seems
            // to be the only way to reset libraries without creating a new instance.
            emulator.libraries = Dict::new();
        } else {
            let root = parse_boc(libs_boc)?;
            emulator.libraries = Dict::from_raw(Some(root));
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_set_debug_enabled(
    transaction_emulator: *mut c_void,
    debug_enabled: bool,
) -> bool {
    ffi_run(|| {
        let emulator = ffi_cast_mut::<TxEmulatorExt>(transaction_emulator)?;
        emulator.debug_enabled = debug_enabled;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_set_prev_blocks_info(
    transaction_emulator: *mut c_void,
    info_boc: *const c_char,
) -> bool {
    ffi_run(|| {
        let emulator = ffi_cast_mut::<TxEmulatorExt>(transaction_emulator)?;
        if info_boc.is_null() {
            return Ok(());
        }

        let info_cell = parse_boc(info_boc).context("Failed to deserialize previous blocks boc")?;
        let info_value = Stack::load_stack_value_from_cell(info_cell.as_ref())
            .context("Failed to deserialize previous blocks tuple")?;

        if info_value.is_null() {
            emulator.prev_blocks_info = None;
        } else if let Ok(tuple) = info_value.into_tuple() {
            emulator.prev_blocks_info = Some(tuple);
        } else {
            anyhow::bail!("Failed to set previous blocks tuple: not a tuple");
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_emulate_transaction(
    transaction_emulator: *mut c_void,
    shard_account_boc: *const c_char,
    message_boc: *const c_char,
) -> *mut c_char {
    ffi_run_with_response(|| {
        let emulator = ffi_cast_mut::<TxEmulatorExt>(transaction_emulator)?;
        let message = parse_boc(message_boc)?;

        let account = parse_boc(shard_account_boc)?
            .parse::<ShardAccount>()
            .context("Failed to unpack shard account")?;

        emulator.emulate(account, TxEmulatorInput::Ordinary(message))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn transaction_emulator_emulate_tick_tock_transaction(
    transaction_emulator: *mut c_void,
    shard_account_boc: *const c_char,
    is_tock: bool,
) -> *mut c_char {
    ffi_run_with_response(|| {
        let emulator = ffi_cast_mut::<TxEmulatorExt>(transaction_emulator)?;

        let account = parse_boc(shard_account_boc)?
            .parse::<ShardAccount>()
            .context("Failed to unpack shard account")?;

        emulator.emulate(account, TxEmulatorInput::TickTock { is_tock })
    })
}

// === TVM Emulator ===

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tvm_emulator_create(
    code_boc: *const c_char,
    data_boc: *const c_char,
    vm_log_verbosity: c_int,
) -> *mut c_void {
    ffi_new::<TvmEmulator, _>(|| {
        let code = parse_boc(code_boc).context("Failed to deserialize code boc")?;
        let data = parse_boc(data_boc).context("Failed to deserialize data boc")?;
        Ok(Box::new(TvmEmulator::new(code, data, vm_log_verbosity)))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tvm_emulator_destroy(tvm_emulator: *mut c_void) {
    ffi_drop::<TvmEmulator>(tvm_emulator);
}

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tvm_emulator_set_extra_currencies(
    _tvm_emulator: *mut c_void,
    _extra_currencies: *const c_char,
) -> bool {
    // TODO: implement
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tvm_emulator_set_config_object(
    tvm_emulator: *mut c_void,
    config: *mut c_void,
) -> bool {
    ffi_run(|| {
        let config = ffi_cast::<tvm_emulator::ParsedConfig>(config)?;
        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;
        emulator.args.config = Some(config.clone());
        Ok(())
    })
}

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tvm_emulator_set_gas_limit(
    tvm_emulator: *mut c_void,
    gas_limit: i64,
) -> bool {
    ffi_run(|| {
        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;
        emulator.set_gas_limit(gas_limit as u64);
        Ok(())
    })
}

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tvm_emulator_run_get_method(
    tvm_emulator: *mut c_void,
    method_id: c_int,
    stack_boc: *const c_char,
) -> *mut c_char {
    ffi_run_with_response::<RunGetMethodResponse, _>(|| {
        let stack_cell = parse_boc(stack_boc).context("Failed to deserialize stack cell")?;
        let stack = stack_cell
            .parse::<Stack>()
            .context("Failed to deserialize stack")?;

        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;

        let subscriber = emulator.make_logger();
        let vm_log = subscriber.state().clone();
        let _tracing = tracing::subscriber::set_default(subscriber);

        let res = emulator.run_get_method(method_id, stack);

        Ok(RunGetMethodResponse {
            success: JsonBool,
            stack: res.stack,
            gas_used: res.gas_used,
            vm_exit_code: res.exit_code,
            vm_log,
            missing_library: res.missing_library,
            debug_log: res.debug_log,
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tvm_emulator_emulate_run_method(
    len: u32,
    params_boc: *const c_char,
    gas_limit: i64,
) -> *mut c_char {
    ffi_run_with_boc(|| {
        let params_boc = std::slice::from_raw_parts(params_boc.cast::<u8>(), len as _);
        let params_cell = Boc::decode(params_boc)?;

        let mut cs = params_cell.as_slice()?;
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
            let mut emulator = TvmEmulator::new(code, data, 3);

            emulator.set_gas_limit(gas_limit as u64);
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

#[repr(C)]
pub struct TvmEulatorEmulateRunMethodResponse {
    pub response: *mut c_char,
    // NOTE: Not really set even in native impl due to verbosity level 0.
    pub log: *mut c_char,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tvm_emulator_emulate_run_method_detailed(
    len: u32,
    params_boc: *const c_char,
    gas_limit: i64,
) -> *mut c_void {
    let response = tvm_emulator_emulate_run_method(len, params_boc, gas_limit);
    if response.is_null() {
        return std::ptr::null_mut();
    }

    let mut res = Box::<TvmEulatorEmulateRunMethodResponse>::new_uninit();

    let res_ptr = res.as_mut_ptr();
    (&raw mut (*res_ptr).response).write(response);
    (&raw mut (*res_ptr).log).write(make_c_str(""));

    Box::into_raw(res).cast()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn run_method_detailed_result_destroy(detailed_result: *mut c_void) {
    if detailed_result.is_null() {
        return;
    }

    let detailed_result = detailed_result.cast::<TvmEulatorEmulateRunMethodResponse>();
    let response_ptr = &raw mut (*detailed_result).response;
    if !response_ptr.is_null() {
        libc::free((*response_ptr).cast());
    }
    let log_ptr = &raw mut (*detailed_result).log;
    if !log_ptr.is_null() {
        libc::free((*log_ptr).cast());
    }

    _ = Box::from_raw(detailed_result);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tvm_emulator_send_external_message(
    tvm_emulator: *mut c_void,
    message_body_boc: *const c_char,
) -> *mut c_char {
    ffi_run_with_response::<TvmEmulatorSendMessageResponse, _>(|| {
        let message_body_cell =
            parse_boc(message_body_boc).context("Failed to parse message body boc")?;

        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;

        let subscriber = emulator.make_logger();
        let vm_log = subscriber.state().clone();
        let _tracing = tracing::subscriber::set_default(subscriber);

        let res = emulator.send_external_message(message_body_cell);

        Ok(TvmEmulatorSendMessageResponse {
            success: JsonBool,
            gas_used: res.gas_used,
            vm_exit_code: res.exit_code,
            accepted: res.accepted,
            vm_log,
            missing_library: res.missing_library,
            actions: res.actions,
            new_code: res.code,
            new_data: res.data,
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tvm_emulator_send_internal_message(
    tvm_emulator: *mut c_void,
    message_body_boc: *const c_char,
    amount: u64,
) -> *mut c_char {
    ffi_run_with_response::<TvmEmulatorSendMessageResponse, _>(|| {
        let message_body_cell =
            parse_boc(message_body_boc).context("Failed to parse message body boc")?;

        let emulator = ffi_cast_mut::<TvmEmulator>(tvm_emulator)?;

        let subscriber = emulator.make_logger();
        let vm_log = subscriber.state().clone();
        let _tracing = tracing::subscriber::set_default(subscriber);

        let res = emulator.send_internal_message(message_body_cell, amount);

        Ok(TvmEmulatorSendMessageResponse {
            success: JsonBool,
            gas_used: res.gas_used,
            vm_exit_code: res.exit_code,
            accepted: res.accepted,
            vm_log,
            missing_library: res.missing_library,
            actions: res.actions,
            new_code: res.code,
            new_data: res.data,
        })
    })
}

// === Utils ===

struct TxEmulatorExt {
    base: TxEmulator,
    block_unixtime: u32,
    lt: u64,
    libraries: Dict<HashBytes, LibDescr>,
    prev_blocks_info: Option<SafeRc<Tuple>>,
    debug_enabled: bool,
}

impl TxEmulatorExt {
    fn rebuild_executor(&mut self, config: &tvm_emulator::ParsedConfig) -> Result<()> {
        let new = TxEmulator::new(config.params.clone(), self.base.verbosity)?;
        self.base.config = new.config;
        self.base.vm_modifiers.signature_with_id = new.vm_modifiers.signature_with_id;
        Ok(())
    }

    fn make_params(&self) -> tycho_executor::ExecutorParams {
        tycho_executor::ExecutorParams {
            libraries: self.libraries.clone(),
            rand_seed: self.base.rand_seed,
            block_unixtime: self.block_unixtime,
            block_lt: self.lt,
            vm_modifiers: self.base.vm_modifiers,
            disable_delete_frozen_accounts: true,
            charge_action_fees_on_fail: true,
            full_body_in_bounced: true,
            strict_extra_currency: true,
            authority_marks_enabled: true,
            // Will be overwritten
            prev_mc_block_id: None,
        }
    }

    fn emulate(
        &mut self,
        account: ShardAccount,
        input: TxEmulatorInput,
    ) -> Result<TxEmulatorResponse> {
        let is_external;
        let is_tock;
        let message = match input {
            TxEmulatorInput::Ordinary(msg) => {
                let msg_info = msg
                    .parse::<MsgInfo>()
                    .context("Failed to unpack message info")?;

                is_external = msg_info.is_external_in();
                is_tock = false;
                Some((msg, msg_info))
            }
            TxEmulatorInput::TickTock { is_tock: input } => {
                is_external = false;
                is_tock = input;
                None
            }
        };

        let IntAddr::Std(address) =
            (match account.load_account().context("Failed to unpack account")? {
                Some(account) => account.address,
                None => match message.as_ref().map(|(_, info)| info) {
                    Some(MsgInfo::Int(info)) => info.dst.clone(),
                    Some(MsgInfo::ExtIn(info)) => info.dst.clone(),
                    Some(MsgInfo::ExtOut(_)) => {
                        anyhow::bail!("Only internal and external inbound messages are accepted");
                    }
                    None => anyhow::bail!("Can't run tick-tock transaction on account_none"),
                },
            })
        else {
            anyhow::bail!("var_addr is not supported");
        };

        let debug_enabled = self.debug_enabled;
        let mut params = self.make_params();
        if params.block_unixtime == 0 {
            params.block_unixtime = now_sec_u64() as u32
        };

        self.base
            .config
            .update_storage_prices(params.block_unixtime)
            .context("Failed to unpack storage prices")?;

        let mut debug_log = String::new();
        let mut inspector = tycho_executor::ExecutorInspector {
            debug: debug_enabled.then_some(&mut debug_log),
            ..Default::default()
        };

        let subscriber = self.base.make_logger();
        let vm_log = subscriber.state().clone();
        let _tracing = tracing::subscriber::set_default(subscriber);

        let output = match message {
            Some((msg_root, _)) => tycho_executor::Executor::new(&params, &self.base.config)
                .with_min_lt(params.block_lt)
                .begin_ordinary_ext(
                    &address,
                    is_external,
                    msg_root,
                    &account,
                    Some(&mut inspector),
                ),
            None => {
                let ty = if is_tock {
                    TickTock::Tock
                } else {
                    TickTock::Tick
                };
                tycho_executor::Executor::new(&params, &self.base.config)
                    .with_min_lt(params.block_lt)
                    .begin_tick_tock_ext(&address, ty, &account, Some(&mut inspector))
            }
        };

        let res = 'res: {
            let output = match output {
                Ok(uncommitted) => uncommitted
                    .commit()
                    .context("Failed to commit transaction")?,
                Err(tycho_executor::TxError::Skipped) if is_external => {
                    break 'res TxEmulatorResponse::NotAccepted(TxEmulatorMsgNotAcceptedResponse {
                        success: JsonBool,
                        error: "External message not accepted by smart contract",
                        external_not_accepted: JsonBool,
                        vm_log,
                        vm_exit_code: inspector.exit_code.unwrap_or(0),
                        debug_log,
                    });
                }
                Err(e) => anyhow::bail!("Fatal executor error: {e:?}"),
            };

            TxEmulatorResponse::Success(TxEmulatorSuccessResponse {
                success: JsonBool,
                transaction: output.transaction.into_inner(),
                shard_account: output.new_state,
                vm_log,
                actions: inspector.actions,
                debug_log,
            })
        };

        Ok(res)
    }
}

enum TxEmulatorInput {
    Ordinary(Cell),
    TickTock { is_tock: bool },
}

fn log_error<T: std::fmt::Debug>(e: T) {
    if VERBOSITY_LEVEL.load(Ordering::Relaxed) > 0 {
        eprintln!("{e:?}");
    }
}

#[inline]
unsafe fn ffi_new<T, F>(f: F) -> *mut c_void
where
    F: FnOnce() -> Result<Box<T>>,
{
    match f() {
        Ok(res) => Box::into_raw(res).cast(),
        Err(e) => {
            log_error(e);
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
            log_error(e);
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
                log_error("the resuling BOC is too big");
                return std::ptr::null_mut();
            };

            // SAFETY: `boc_len` is in `isize::MAX` bounds.
            let res = unsafe { libc::malloc(4 + boc_len as usize) };
            if res.is_null() {
                return res.cast();
            }

            // SAFETY: `res` is not null and the allocated range is enough.
            unsafe {
                std::ptr::copy_nonoverlapping(boc_len.to_le_bytes().as_ptr(), res.cast::<u8>(), 4);
                std::ptr::copy_nonoverlapping(boc.as_ptr(), res.cast::<u8>().add(4), boc.len());
            }

            res.cast()
        }
        Err(e) => {
            log_error(e);
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
    anyhow::ensure!(!boc_str.is_null(), "String pointer is null");
    let boc_str = CStr::from_ptr(boc_str).to_str()?;
    Boc::decode_base64(boc_str).map_err(Into::into)
}

unsafe fn parse_config(boc_str: *const c_char) -> Result<tvm_emulator::ParsedConfig> {
    parse_boc(boc_str).and_then(tvm_emulator::ParsedConfig::try_from_root)
}

unsafe fn parse_std_addr(addr_str: *const c_char) -> Result<StdAddr> {
    anyhow::ensure!(!addr_str.is_null(), "String pointer is null");
    let addr_str = CStr::from_ptr(addr_str).to_str()?;
    addr_str.parse::<StdAddr>().map_err(Into::into)
}

unsafe fn parse_hash(hash_str: *const c_char) -> Result<HashBytes> {
    anyhow::ensure!(!hash_str.is_null(), "String pointer is null");
    let hash_str = CStr::from_ptr(hash_str).to_str()?;
    hash_str.parse::<HashBytes>().map_err(Into::into)
}

/// Allocates a new C-string with `malloc`.
#[cfg(unix)]
fn make_c_str(str: &str) -> *mut c_char {
    // SAFETY: `str` is not null and its len is in `isize::MAX` bounds.
    unsafe { libc::strndup(str.as_ptr().cast(), str.len()) }
}

/// Allocates a new C-string with `malloc`.
#[cfg(windows)]
fn make_c_str(str: &str) -> *mut c_char {
    // SAFETY: `str` is not null and its len is in `isize::MAX` bounds.
    unsafe {
        let len = str.len();
        let result = libc::malloc(len + 1).cast::<c_char>();

        if !result.is_null() {
            std::ptr::copy_nonoverlapping(str.as_ptr().cast(), result, len);
            result.add(len).write(0);
        }

        result
    }
}

#[cfg(test)]
mod test {
    use tycho_vm::{SmcInfo, SmcInfoBase};

    use super::*;

    #[test]
    fn tvm_run_get_method() -> anyhow::Result<()> {
        let code = Boc::decode_base64(
            "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",
        )?;
        let data = Boc::decode_base64(
            "te6ccgECEgEAApQAAZMAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAwAr8PZg40Acl6JLMYh7tIHVhdRh3EgOwNZD64cQZwso8mAEBa4AVJCimjkbNb/3dgqZHUta7ni4y+RG4Gorxv0pSAXFGeWAAAAGT5ZOaCM6dnPJnq0AAAAAAMAIBEv8A9KQT9LzyCwMCASAFBAOa8n/tRNDXScMB+GaJ+Gkh2zzTAAGOFIMI1xgg+CjIzs7J+QBY+EL5EPKo3tM/AfhDIbnytCD4I4ED6KiCCBt3QKC58rT4Y9MfAds88jwQDggCAsUHBgETsgIMNs8+A/yAIAsDVdjh2omhrpOGA/DMRaGmB/SAYfDTUnABuEOOAcYEQ64aP+V4Q8YGA7Z55HkREQgBFCCCEBWgOPu64wIJBHYw+EJu4wD4RvJzIZbU0x/U0dCT1NMf4vpA1NHQ+kDR+En4SscFII8SMCGJxwWzII6IMCHbPPhJxwXe3w4QDQoCQI6FVHMg2zyOECDIz4UIzoBvz0DJgQCg+wDiXwTbPPIADAsALPhK+EP4QsjL/8s/z4PO+EvIzs3J7VQARPhKyM74S88WgQCgz0ASyx/O+CoBzCH7BAHQ7R7tU8nxGAgAasiDB89AcG2AQPRD+EpxWIBA9BZyAYBA9BbI9ADJ+CrIz4SA9AD0AM+ByfkAyM+KAEDL/8nQA27tRNDXScIBjyxw7UTQ9AVxIYBA9A6OgYnfciKAQPQOjoGJ3/hr+GqAQPQO8r3XC//4YnD4Y+MNEBAPADbtRNDT/9M/0wD6QNTR0PpA0fhr+Gr4Zvhj+GIAQ4AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABAACvhG8uBM",
        )?;

        let method_id = tycho_types::crc::crc_16(b"get_wallet_data") as u32 | 0x10000;

        let smc_info = SmcInfoBase::new()
            .with_now(now_sec_u64() as u32)
            .with_block_lt(0)
            .with_tx_lt(0)
            .with_account_addr(
                "0:5ee27bd184049818ff87ff88d25867c47a5d24f38ae40852da17f0b6d51e990d"
                    .parse::<StdAddr>()?
                    .into(),
            )
            .require_ton_v4()
            .with_code(code.clone())
            .build_c7();
        let c7 = tycho_vm::Stack::with_items(vec![smc_info.into_dyn_value()]);

        let params = CellBuilder::build_from((
            method_id,
            code,
            data,
            CellBuilder::build_from(tycho_vm::Stack::default())?,
            CellBuilder::build_from((CellBuilder::build_from(c7)?, ()))?,
        ))
        .map(Boc::encode)?;

        let res = unsafe {
            tvm_emulator_emulate_run_method(params.len() as u32, params.as_ptr().cast(), 100000)
        };
        assert!(!res.is_null());

        let len = u32::from_le_bytes(unsafe { *res.cast::<[u8; 4]>() });
        println!("result len: {}", len);

        let data = Boc::decode(unsafe {
            std::slice::from_raw_parts(res.add(4).cast::<u8>(), len as usize)
        })?;

        unsafe { string_destroy(res) };

        let mut cs = data.as_slice()?;
        let exit_code = cs.load_u32()? as i32;
        let gas_used = cs.load_u64()?;
        let stack = cs.load_reference()?.parse::<tycho_vm::Stack>()?;
        println!("exit_code={exit_code}, gas_used={gas_used}");
        println!("stack={stack:?}");

        Ok(())
    }
}
