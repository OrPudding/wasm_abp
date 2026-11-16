use crate::component;
use crate::ffi_support::{clear_last_error, guarded, last_error, set_last_error};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::slice;

#[no_mangle]
pub extern "C" fn wasm_abp_init() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn wasm_abp_version() -> *const c_char {
    static VERSION: &[u8] = b"wasm_abp ABI 1 (Wasmtime 50.0.1)\0";
    VERSION.as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn wasm_abp_get_last_error() -> *mut c_char {
    last_error()
        .and_then(|error| CString::new(error).ok())
        .map_or(std::ptr::null_mut(), CString::into_raw)
}

#[no_mangle]
pub extern "C" fn wasm_abp_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_abp_free_bytes(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        unsafe {
            let _ = Box::from_raw(slice::from_raw_parts_mut(ptr, len));
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_abp_abi_version() -> u32 {
    1
}

#[no_mangle]
pub extern "C" fn wasm_abp_diagnostics() -> *mut c_char {
    CString::new(crate::component::diagnostics().to_string())
        .unwrap()
        .into_raw()
}

#[no_mangle]
pub extern "C" fn wasm_abp_configure_cache(path: *const c_char) -> i32 {
    clear_last_error();
    let result = guarded(|| {
        if path.is_null() {
            return Err("Cache path is null".into());
        }
        let path = unsafe { CStr::from_ptr(path) }
            .to_str()
            .map_err(|error| error.to_string())?;
        component::configure_cache(path)
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
pub extern "C" fn wasm_abp_component_compile(bytes_ptr: *const u8, bytes_len: usize) -> i64 {
    clear_last_error();
    if bytes_ptr.is_null() || bytes_len == 0 {
        set_last_error("null or empty component bytes".to_string());
        return -1;
    }
    let slice = unsafe { slice::from_raw_parts(bytes_ptr, bytes_len) };
    match component::compile_component(slice) {
        Ok(handle) => handle,
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_abp_component_compile_for_api(
    bytes_ptr: *const u8,
    bytes_len: usize,
    api_level: u32,
) -> i64 {
    clear_last_error();
    if bytes_ptr.is_null() || bytes_len == 0 {
        set_last_error("null or empty component bytes".to_string());
        return -1;
    }
    let bytes = unsafe { slice::from_raw_parts(bytes_ptr, bytes_len) };
    match guarded(|| component::compile_component_for_api(bytes, api_level)) {
        Ok(handle) => handle,
        Err(error) => {
            set_last_error(error);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_abp_component_precompile(
    bytes_ptr: *const u8,
    bytes_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    clear_last_error();
    if bytes_ptr.is_null() || bytes_len == 0 || out_ptr.is_null() || out_len.is_null() {
        set_last_error("invalid argument".to_string());
        return -1;
    }
    let slice = unsafe { slice::from_raw_parts(bytes_ptr, bytes_len) };
    match component::precompile_component(slice) {
        Ok(cwasm) => {
            let mut boxed = cwasm.into_boxed_slice();
            unsafe {
                *out_ptr = boxed.as_mut_ptr();
                *out_len = boxed.len();
            }
            std::mem::forget(boxed);
            0
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_abp_component_deserialize(bytes_ptr: *const u8, bytes_len: usize) -> i64 {
    clear_last_error();
    if bytes_ptr.is_null() || bytes_len == 0 {
        set_last_error("null or empty cwasm bytes".to_string());
        return -1;
    }
    let slice = unsafe { slice::from_raw_parts(bytes_ptr, bytes_len) };
    match component::deserialize_component(slice) {
        Ok(handle) => handle,
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_abp_component_instantiate(
    component_handle: i64,
    preopens_json: *const c_char,
) -> i64 {
    clear_last_error();
    // preopens 为 guest/host 路径映射的 JSON 数组，null 表示空映射
    let mut preopens = Vec::new();
    if !preopens_json.is_null() {
        let text = match unsafe { CStr::from_ptr(preopens_json) }.to_str() {
            Ok(s) => s,
            Err(e) => {
                set_last_error(e.to_string());
                return -1;
            }
        };
        match serde_json::from_str::<Vec<std::collections::HashMap<String, String>>>(text) {
            Ok(rows) => {
                for row in rows {
                    if let (Some(guest), Some(host)) = (row.get("guest"), row.get("host")) {
                        preopens.push((guest.clone(), host.clone()));
                    }
                }
            }
            Err(e) => {
                set_last_error(format!("invalid preopens: {e}"));
                return -1;
            }
        }
    }
    match guarded(|| component::instantiate_component(component_handle, &preopens)) {
        Ok(handle) => handle,
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_abp_component_instance_drop(handle: i64) -> i32 {
    if component::drop_component_instance(handle) {
        0
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn wasm_abp_component_drop(handle: i64) -> i32 {
    if component::drop_component(handle) {
        0
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn wasm_abp_component_submit(handle: i64, json: *const c_char) -> i32 {
    component_json_command(handle, json, component::submit_component_call)
}

#[no_mangle]
pub extern "C" fn wasm_abp_component_respond(handle: i64, json: *const c_char) -> i32 {
    component_json_command(handle, json, component::respond_component_host)
}

fn component_json_command(
    handle: i64,
    json: *const c_char,
    action: fn(i64, &str) -> Result<(), String>,
) -> i32 {
    match guarded(|| {
        if json.is_null() {
            return Err("Missing component message".to_string());
        }
        let json = unsafe { CStr::from_ptr(json) }
            .to_str()
            .map_err(|e| e.to_string())?;
        action(handle, json)
    }) {
        Ok(()) => 0,
        Err(error) => {
            set_last_error(error);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_abp_component_poll(handle: i64) -> *mut c_char {
    match guarded(|| component::poll_component_events(handle)) {
        Ok(json) => CString::new(json).unwrap().into_raw(),
        Err(error) => {
            set_last_error(error);
            std::ptr::null_mut()
        }
    }
}
