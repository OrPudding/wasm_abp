use crate::engine;
use crate::ffi_support::{clear_last_error, guarded, last_error, set_last_error};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::slice;
use wasmtime::Val;

#[no_mangle]
pub extern "C" fn wasm_core_init() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn wasm_core_version() -> *const c_char {
    static VERSION: &[u8] = b"wasm_core ABI 1 (Wasmtime 50.0.1, WASI Preview 1)\0";
    VERSION.as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn wasm_core_get_last_error() -> *mut c_char {
    last_error()
        .and_then(|error| CString::new(error).ok())
        .map_or(std::ptr::null_mut(), CString::into_raw)
}

#[no_mangle]
pub extern "C" fn wasm_core_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_core_free_bytes(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        unsafe {
            let _ = Box::from_raw(slice::from_raw_parts_mut(ptr, len));
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_core_abi_version() -> u32 {
    1
}

#[no_mangle]
pub extern "C" fn wasm_core_diagnostics() -> *mut c_char {
    CString::new(crate::engine::diagnostics().to_string())
        .unwrap()
        .into_raw()
}

#[no_mangle]
pub extern "C" fn wasm_core_configure_cache(path: *const c_char) -> i32 {
    clear_last_error();
    let result = guarded(|| {
        if path.is_null() {
            return Err("Cache path is null".into());
        }
        let path = unsafe { CStr::from_ptr(path) }
            .to_str()
            .map_err(|error| error.to_string())?;
        crate::compiled_cache::configure(path)
    });
    match result {
        Ok(()) => 0,
        Err(error) => {
            set_last_error(error);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_core_module_compile(bytes_ptr: *const u8, bytes_len: usize) -> i64 {
    clear_last_error();
    if bytes_ptr.is_null() || bytes_len == 0 {
        set_last_error("null or empty wasm buffer".to_string());
        return -1;
    }
    let slice = unsafe { slice::from_raw_parts(bytes_ptr, bytes_len) };
    match guarded(|| engine::compile_module(slice)) {
        Ok(handle) => handle,
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_core_module_instantiate(module_handle: i64, with_wasi: i32) -> i64 {
    clear_last_error();
    match guarded(|| engine::instantiate_module(module_handle, with_wasi != 0)) {
        Ok(handle) => handle,
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_core_module_instantiate_config(
    module_handle: i64,
    with_wasi: i32,
    preopens: *const c_char,
) -> i64 {
    clear_last_error();
    let result = guarded(|| {
        if preopens.is_null() {
            return Err("preopens are null".into());
        }
        let json = unsafe { CStr::from_ptr(preopens) }
            .to_str()
            .map_err(|error| error.to_string())?;
        let paths = serde_json::from_str(json).map_err(|error| error.to_string())?;
        engine::instantiate_module_with_preopens(module_handle, with_wasi != 0, &paths)
    });
    match result {
        Ok(handle) => handle,
        Err(error) => {
            set_last_error(error);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_core_instance_call_json(
    instance: i64,
    name: *const c_char,
    arguments: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let result = guarded(|| {
        if name.is_null() || arguments.is_null() {
            return Err("call arguments are null".into());
        }
        let name = unsafe { CStr::from_ptr(name) }
            .to_str()
            .map_err(|error| error.to_string())?;
        let json = unsafe { CStr::from_ptr(arguments) }
            .to_str()
            .map_err(|error| error.to_string())?;
        let args = serde_json::from_str::<Vec<serde_json::Value>>(json)
            .map_err(|error| error.to_string())?;
        engine::call_function_json(instance, name, &args)
    });
    match result {
        Ok(value) => CString::new(value.to_string()).unwrap().into_raw(),
        Err(error) => {
            set_last_error(error);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_core_instance_call(
    instance_handle: i64,
    func_name: *const c_char,
    args_ptr: *const i64,
    args_len: usize,
    results_ptr: *mut i64,
    max_results: usize,
    out_results_count: *mut usize,
) -> i32 {
    clear_last_error();
    if func_name.is_null() {
        set_last_error("function name is null".to_string());
        return -1;
    }
    let name_str = match unsafe { CStr::from_ptr(func_name) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e.to_string());
            return -1;
        }
    };

    let raw_args = if !args_ptr.is_null() && args_len > 0 {
        unsafe { slice::from_raw_parts(args_ptr, args_len) }
    } else {
        &[]
    };

    match guarded(|| engine::call_function_dynamic(instance_handle, name_str, raw_args)) {
        Ok(return_vals) => {
            let count = return_vals.len();
            if !results_ptr.is_null() && max_results > 0 {
                let to_copy = count.min(max_results);
                let out_slice = unsafe { slice::from_raw_parts_mut(results_ptr, to_copy) };
                for (i, val) in return_vals.iter().take(to_copy).enumerate() {
                    match val {
                        Val::I32(v) => out_slice[i] = *v as i64,
                        Val::I64(v) => out_slice[i] = *v,
                        Val::F32(v) => out_slice[i] = f32::from_bits(*v) as i64,
                        Val::F64(v) => out_slice[i] = f64::from_bits(*v) as i64,
                        _ => out_slice[i] = 0,
                    }
                }
            }
            if !out_results_count.is_null() {
                unsafe {
                    *out_results_count = count;
                }
            }
            0
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_core_instance_read_memory(
    instance_handle: i64,
    mem_name: *const c_char,
    offset: usize,
    out_buf: *mut u8,
    len: usize,
) -> i32 {
    clear_last_error();
    if mem_name.is_null() || out_buf.is_null() {
        set_last_error("invalid arguments to read_memory".to_string());
        return -1;
    }
    let name_str = match unsafe { CStr::from_ptr(mem_name) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e.to_string());
            return -1;
        }
    };
    let slice = unsafe { slice::from_raw_parts_mut(out_buf, len) };
    match guarded(|| engine::read_memory(instance_handle, name_str, offset, slice)) {
        Ok(()) => 0,
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_core_instance_write_memory(
    instance_handle: i64,
    mem_name: *const c_char,
    offset: usize,
    in_buf: *const u8,
    len: usize,
) -> i32 {
    clear_last_error();
    if mem_name.is_null() || in_buf.is_null() {
        set_last_error("invalid arguments to write_memory".to_string());
        return -1;
    }
    let name_str = match unsafe { CStr::from_ptr(mem_name) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e.to_string());
            return -1;
        }
    };
    let slice = unsafe { slice::from_raw_parts(in_buf, len) };
    match guarded(|| engine::write_memory(instance_handle, name_str, offset, slice)) {
        Ok(()) => 0,
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_core_instance_drop(handle: i64) -> i32 {
    if engine::drop_instance(handle) {
        0
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn wasm_core_module_drop(handle: i64) -> i32 {
    if engine::drop_module(handle) {
        0
    } else {
        -1
    }
}
