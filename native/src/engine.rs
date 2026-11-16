use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use wasmtime::*;
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::{FsPerms, WasiCtxBuilder};

static COMPILED: OnceLock<crate::compiled_cache::CompilationCache<Module>> = OnceLock::new();

static GLOBAL_ENGINE: OnceLock<Engine> = OnceLock::new();

pub fn get_engine() -> &'static Engine {
    GLOBAL_ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.wasm_memory64(false);
        config.memory_may_move(true);
        const MAX_PLUGIN_MEMORY: u64 = 128 * 1024 * 1024; // 128 MiB
        config.memory_reservation(MAX_PLUGIN_MEMORY);
        config.memory_reservation_for_growth(MAX_PLUGIN_MEMORY);
        config.cranelift_opt_level(OptLevel::Speed);
        #[cfg(target_os = "ios")]
        config
            .target("pulley64")
            .expect("failed to select iOS interpreter target");
        Engine::new(&config).expect("failed to initialize Wasmtime Engine")
    })
}

pub struct ModuleEntry {
    pub module: Arc<Module>,
    pub source_bytes: usize,
}

pub struct InstanceEntry {
    pub store: Store<InstanceContext>,
    pub instance: Instance,
    _module: Arc<Module>,
}

pub struct InstanceContext {
    pub wasi_ctx: Option<WasiP1Ctx>,
}

static MODULE_TABLE: OnceLock<Mutex<HashMap<i64, Arc<ModuleEntry>>>> = OnceLock::new();
static INSTANCE_TABLE: OnceLock<Mutex<HashMap<i64, Arc<Mutex<InstanceEntry>>>>> = OnceLock::new();
static NEXT_HANDLE: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);

fn next_handle() -> i64 {
    NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn modules() -> &'static Mutex<HashMap<i64, Arc<ModuleEntry>>> {
    MODULE_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn instances() -> &'static Mutex<HashMap<i64, Arc<Mutex<InstanceEntry>>>> {
    INSTANCE_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn compile_module(wasm_bytes: &[u8]) -> Result<i64, String> {
    let engine = get_engine();
    let module = COMPILED.get_or_init(Default::default).acquire(
        wasm_bytes,
        engine.precompile_compatibility_hash(),
        "core",
        |bytes| unsafe { Module::deserialize(engine, bytes) }.map_err(|error| format!("{error:#}")),
        || Module::from_binary(engine, wasm_bytes).map_err(|error| format!("{error:#}")),
        |module| module.serialize().map_err(|error| error.to_string()),
    )?;
    let handle = next_handle();
    let entry = Arc::new(ModuleEntry {
        module,
        source_bytes: wasm_bytes.len(),
    });
    modules().lock().unwrap().insert(handle, entry);
    Ok(handle)
}

pub fn instantiate_module(module_handle: i64, with_wasi: bool) -> Result<i64, String> {
    instantiate_module_with_preopens(module_handle, with_wasi, &HashMap::new())
}

pub fn instantiate_module_with_preopens(
    module_handle: i64,
    with_wasi: bool,
    preopens: &HashMap<String, String>,
) -> Result<i64, String> {
    let module_entry = {
        let guard = modules().lock().unwrap();
        guard
            .get(&module_handle)
            .cloned()
            .ok_or("invalid module handle")?
    };

    let engine = get_engine();
    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder.inherit_stdout().inherit_stderr();
    if with_wasi {
        for (guest, host) in preopens {
            let permissions = if guest == "/plugin" {
                FsPerms::ReadOnly
            } else {
                FsPerms::ReadWrite
            };
            wasi_builder
                .preopened_dir(host, guest, permissions)
                .map_err(|error| error.to_string())?;
        }
    }
    let wasi_ctx = wasi_builder.build_p1();

    let mut store = Store::new(
        engine,
        InstanceContext {
            wasi_ctx: Some(wasi_ctx),
        },
    );
    let mut linker = Linker::new(engine);
    if with_wasi {
        p1::add_to_linker_sync(&mut linker, |ctx: &mut InstanceContext| {
            ctx.wasi_ctx.as_mut().expect("wasi context missing")
        })
        .map_err(|e| e.to_string())?;
    }
    let instance = linker
        .instantiate(&mut store, &module_entry.module)
        .map_err(|e| format!("{e:#}"))?;

    let handle = next_handle();
    instances().lock().unwrap().insert(
        handle,
        Arc::new(Mutex::new(InstanceEntry {
            store,
            instance,
            _module: module_entry.module.clone(),
        })),
    );
    Ok(handle)
}

pub fn call_function_dynamic(
    instance_handle: i64,
    func_name: &str,
    raw_args: &[i64],
) -> Result<Vec<Val>, String> {
    let instance = get_instance(instance_handle)?;
    let mut guard = instance.lock().unwrap();
    let entry = &mut *guard;

    let func = entry
        .instance
        .get_func(&mut entry.store, func_name)
        .ok_or_else(|| format!("function '{}' not found", func_name))?;

    let fn_ty = func.ty(&entry.store);
    let param_types = fn_ty.params().collect::<Vec<_>>();
    if raw_args.len() != param_types.len() {
        return Err(format!(
            "function '{}' expects {} arguments, got {}",
            func_name,
            param_types.len(),
            raw_args.len()
        ));
    }

    let mut converted_params = Vec::with_capacity(param_types.len());
    for (i, p_ty) in param_types.iter().enumerate() {
        let v = raw_args[i];
        match p_ty {
            ValType::I32 => converted_params.push(Val::I32(v as i32)),
            ValType::I64 => converted_params.push(Val::I64(v)),
            ValType::F32 => converted_params.push(Val::F32(v as u32)),
            ValType::F64 => converted_params.push(Val::F64(v as u64)),
            _ => return Err(format!("unsupported parameter type for arg {}", i)),
        }
    }

    let result_types = fn_ty.results().collect::<Vec<_>>();
    let mut return_vals = vec![Val::I64(0); result_types.len()];

    func.call(&mut entry.store, &converted_params, &mut return_vals)
        .map_err(|e| e.to_string())?;

    Ok(return_vals)
}

pub fn call_function_json(
    instance_handle: i64,
    func_name: &str,
    args: &[serde_json::Value],
) -> Result<serde_json::Value, String> {
    let types = {
        let instance = get_instance(instance_handle)?;
        let mut guard = instance.lock().unwrap();
        let entry = &mut *guard;
        let func = entry
            .instance
            .get_func(&mut entry.store, func_name)
            .ok_or("function not found")?;
        func.ty(&entry.store).params().collect::<Vec<_>>()
    };
    if types.len() != args.len() {
        return Err("incorrect argument count".into());
    }
    let mut raw = Vec::with_capacity(args.len());
    for (value, ty) in args.iter().zip(types) {
        let bits = match ty {
            ValType::I32 | ValType::I64 => value.as_i64().ok_or("integer argument required")?,
            ValType::F32 => {
                (value.as_f64().ok_or("numeric argument required")? as f32).to_bits() as i64
            }
            ValType::F64 => value.as_f64().ok_or("numeric argument required")?.to_bits() as i64,
            _ => return Err("unsupported argument type".into()),
        };
        raw.push(bits);
    }
    let values = call_function_dynamic(instance_handle, func_name, &raw)?;
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        result.push(match value {
            Val::I32(value) => serde_json::json!(value),
            Val::I64(value) => serde_json::json!(value),
            Val::F32(value) => serde_json::Value::Number(
                serde_json::Number::from_f64(f32::from_bits(value) as f64)
                    .ok_or("non-finite numeric result")?,
            ),
            Val::F64(value) => serde_json::Value::Number(
                serde_json::Number::from_f64(f64::from_bits(value))
                    .ok_or("non-finite numeric result")?,
            ),
            _ => return Err("unsupported result type".into()),
        });
    }
    Ok(serde_json::Value::Array(result))
}

pub fn read_memory(
    instance_handle: i64,
    mem_name: &str,
    offset: usize,
    buffer: &mut [u8],
) -> Result<(), String> {
    let instance = get_instance(instance_handle)?;
    let mut guard = instance.lock().unwrap();
    let entry = &mut *guard;

    let memory = entry
        .instance
        .get_memory(&mut entry.store, mem_name)
        .ok_or_else(|| format!("memory '{}' not found", mem_name))?;

    let data = memory.data(&entry.store);
    if offset
        .checked_add(buffer.len())
        .is_none_or(|end| end > data.len())
    {
        return Err("memory read out of bounds".to_string());
    }

    buffer.copy_from_slice(&data[offset..offset + buffer.len()]);
    Ok(())
}

pub fn write_memory(
    instance_handle: i64,
    mem_name: &str,
    offset: usize,
    buffer: &[u8],
) -> Result<(), String> {
    let instance = get_instance(instance_handle)?;
    let mut guard = instance.lock().unwrap();
    let entry = &mut *guard;

    let memory = entry
        .instance
        .get_memory(&mut entry.store, mem_name)
        .ok_or_else(|| format!("memory '{}' not found", mem_name))?;

    let data = memory.data_mut(&mut entry.store);
    if offset
        .checked_add(buffer.len())
        .is_none_or(|end| end > data.len())
    {
        return Err("memory write out of bounds".to_string());
    }

    data[offset..offset + buffer.len()].copy_from_slice(buffer);
    Ok(())
}

fn get_instance(handle: i64) -> Result<Arc<Mutex<InstanceEntry>>, String> {
    instances()
        .lock()
        .unwrap()
        .get(&handle)
        .cloned()
        .ok_or("invalid instance handle".into())
}

pub fn drop_instance(handle: i64) -> bool {
    // An in-flight call owns its Store until completion. Dropping the handle
    // must not hold the process-wide table lock while waiting for guest code.
    let instance = instances().lock().unwrap().remove(&handle);
    instance.is_some()
}

pub fn drop_module(handle: i64) -> bool {
    modules().lock().unwrap().remove(&handle).is_some()
}

pub fn diagnostics() -> serde_json::Value {
    let (module_count, module_source_bytes) = {
        let modules = modules().lock().unwrap();
        (
            modules.len(),
            modules
                .values()
                .map(|entry| entry.source_bytes)
                .sum::<usize>(),
        )
    };
    let entries = instances()
        .lock()
        .unwrap()
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let mut memory_bytes = 0;
    let mut busy_instances = 0;
    for instance in &entries {
        let Ok(mut guard) = instance.try_lock() else {
            busy_instances += 1;
            continue;
        };
        let entry = &mut *guard;
        let memories = entry
            .instance
            .exports(&mut entry.store)
            .filter_map(|export| export.into_memory())
            .collect::<Vec<_>>();
        for memory in memories {
            memory_bytes += memory.data_size(&entry.store);
        }
    }
    serde_json::json!({
        "cache": COMPILED.get_or_init(Default::default).diagnostics(),
        "version": "50.0.1", "engineInitialized": GLOBAL_ENGINE.get().is_some(),
        "modules": module_count, "instances": entries.len(),
        "moduleSourceBytes": module_source_bytes,
        "exportedMemoryBytes": memory_bytes,
        "busyInstances": busy_instances,
    })
}
