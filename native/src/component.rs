use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use wasmtime::component::{Component, Instance, Linker, ResourceTable};
use wasmtime::*;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

static COMPILED: OnceLock<crate::compiled_cache::CompilationCache<Component>> = OnceLock::new();

static ENGINE: OnceLock<Engine> = OnceLock::new();
static LIVE_STORES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn get_engine() -> &'static Engine {
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config
            .wasm_component_model(true)
            .wasm_component_model_async(true);

        config.epoch_interruption(true);
        config.legacy_component_sync_suspension(true);
        #[cfg(target_os = "ios")]
        {
            config
                .target("pulley64")
                .expect("failed to select iOS interpreter target");
            const RESERVE: u64 = 128 << 20;
            config
                .wasm_memory64(false)
                .memory_may_move(true)
                .memory_reservation(RESERVE)
                .memory_reservation_for_growth(RESERVE);
        }
        let engine = Engine::new(&config).expect("failed to create component engine");
        let ticker = engine.clone();
        std::thread::Builder::new()
            .name("abp-epoch".to_string())
            .spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(10));
                ticker.increment_epoch();
            })
            .expect("failed to start component epoch ticker");
        engine
    })
}

pub struct ComponentEntry {
    pub component: Arc<Component>,
}

pub struct ComponentInstanceContext {
    pub bridge: Arc<ComponentBridge>,
    commands: tokio::sync::mpsc::UnboundedSender<ComponentCommand>,
    pub timers: HashMap<u64, tokio::task::JoinHandle<()>>,
    pub next_timer_id: u64,
    pub wasi_ctx: WasiCtx,
    pub http_ctx: WasiHttpCtx,

    pub http_hooks: crate::component_http::HttpHooks,
    pub table: ResourceTable,
    limits: StoreLimits,
    pub rendered_ui: Arc<Mutex<Option<String>>>,
    pub next_node_id: Arc<std::sync::atomic::AtomicU64>,
    _component: Option<Arc<Component>>,
    _lifetime: StoreLifetime,
}

struct StoreLifetime;

impl StoreLifetime {
    fn new() -> Self {
        LIVE_STORES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self
    }
}

impl Drop for StoreLifetime {
    fn drop(&mut self) {
        LIVE_STORES.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

struct ReleaseDriver(std::sync::mpsc::Sender<()>);

impl Drop for ReleaseDriver {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

// Field order makes the acknowledgement follow destruction of the whole future,
// including its Store and compiled component captured outside the Store.
struct ComponentDriver<F> {
    future: std::pin::Pin<Box<F>>,
    _released: ReleaseDriver,
}

impl<F: std::future::Future> std::future::Future for ComponentDriver<F> {
    type Output = F::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.get_mut().future.as_mut().poll(context)
    }
}

#[derive(Default)]
pub struct ComponentBridge {
    events: Mutex<Vec<serde_json::Value>>,
    requests: Mutex<HashMap<u64, tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>>>,
    next_request: std::sync::atomic::AtomicU64,
}

struct PendingHostRequest<'a> {
    bridge: &'a ComponentBridge,
    id: u64,
}

impl Drop for PendingHostRequest<'_> {
    fn drop(&mut self) {
        self.bridge.requests.lock().unwrap().remove(&self.id);
    }
}

impl ComponentBridge {
    pub fn notify(&self, method: &str, args: serde_json::Value) {
        self.emit(serde_json::json!({"type": "host-notify", "method": method, "args": args}));
    }
    pub fn emit(&self, event: serde_json::Value) {
        self.events.lock().unwrap().push(event);
    }

    pub async fn request(
        &self,
        method: String,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = self
            .next_request
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (send, receive) = tokio::sync::oneshot::channel();
        self.requests.lock().unwrap().insert(id, send);
        let _pending = PendingHostRequest { bridge: self, id };
        self.emit(serde_json::json!({"type": "host", "id": id, "method": method, "args": args}));
        receive
            .await
            .map_err(|_| "Plugin host request cancelled".to_string())?
    }
}

impl ComponentInstanceContext {
    pub fn publish_ui(&self, json: String) {
        *self.rendered_ui.lock().unwrap() = Some(json.clone());
        self.bridge
            .emit(serde_json::json!({"type": "ui", "json": json}));
    }
}

impl Drop for ComponentInstanceContext {
    fn drop(&mut self) {
        for (_, timer) in self.timers.drain() {
            timer.abort();
        }
        self.bridge.requests.lock().unwrap().clear();
    }
}

pub fn start_component_timer(
    context: &mut ComponentInstanceContext,
    delay_ms: u64,
    payload: String,
    repeat: bool,
) -> u64 {
    context.next_timer_id += 1;
    let id = context.next_timer_id;
    let commands = context.commands.clone();
    let task = component_executor().spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms.max(1))).await;
            let args = vec![
                wasmtime::component::Val::Enum("timer".to_string()),
                wasmtime::component::Val::String(
                    serde_json::json!({
                        "timerId": id,
                        "kind": if repeat { "interval" } else { "timeout" },
                        "payload": payload,
                    })
                    .to_string(),
                ),
            ];
            if commands
                .send(ComponentCommand::Call {
                    id: 0,
                    name: "on-event".to_string(),
                    args,
                    reply: None,
                    completed_timer: if repeat { None } else { Some(id) },
                })
                .is_err()
                || !repeat
            {
                break;
            }
        }
    });
    context.timers.insert(id, task);
    id
}

impl WasiView for ComponentInstanceContext {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi_ctx,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for ComponentInstanceContext {
    fn http(&mut self) -> wasmtime_wasi_http::WasiHttpCtxView<'_> {
        wasmtime_wasi_http::WasiHttpCtxView {
            ctx: &mut self.http_ctx,
            table: &mut self.table,
            hooks: &mut self.http_hooks,
        }
    }
}

pub struct ComponentInstanceEntry {
    commands: tokio::sync::mpsc::UnboundedSender<ComponentCommand>,
    task: tokio::task::JoinHandle<()>,
    bridge: Arc<ComponentBridge>,
    rendered_ui: Arc<Mutex<Option<String>>>,
    released: std::sync::mpsc::Receiver<()>,
}

enum ComponentCommand {
    Call {
        id: u64,
        name: String,
        args: Vec<wasmtime::component::Val>,
        reply: Option<std::sync::mpsc::Sender<Result<String, String>>>,
        completed_timer: Option<u64>,
    },
}

fn get_component_func(
    instance: &mut wasmtime::component::Instance,
    store: &mut wasmtime::Store<ComponentInstanceContext>,
    interface_names: &[&str],
    func_name: &str,
) -> Option<wasmtime::component::Func> {
    for iface in interface_names {
        let qualified = format!("{iface}#{func_name}");
        if let Some(func) = instance.get_func(&mut *store, &qualified) {
            return Some(func);
        }
        if let Some(inst_idx) = instance.get_export_index(&mut *store, None, iface) {
            if let Some(func_idx) =
                instance.get_export_index(&mut *store, Some(&inst_idx), func_name)
            {
                if let Some(func) = instance.get_func(&mut *store, func_idx) {
                    return Some(func);
                }
            }
        }
    }
    if let Some(func) = instance.get_func(&mut *store, func_name) {
        return Some(func);
    }
    None
}

static COMPONENT_TABLE: OnceLock<Mutex<HashMap<i64, Arc<ComponentEntry>>>> = OnceLock::new();
static COMP_INSTANCE_TABLE: OnceLock<Mutex<HashMap<i64, ComponentInstanceEntry>>> = OnceLock::new();
static NEXT_HANDLE: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(10000);

fn next_handle() -> i64 {
    NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn components() -> &'static Mutex<HashMap<i64, Arc<ComponentEntry>>> {
    COMPONENT_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn instances() -> &'static Mutex<HashMap<i64, ComponentInstanceEntry>> {
    COMP_INSTANCE_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn compile_component(wasm_bytes: &[u8]) -> Result<i64, String> {
    let engine = get_engine();
    let component = COMPILED.get_or_init(Default::default).acquire(
        wasm_bytes,
        engine.precompile_compatibility_hash(),
        "component",
        |bytes| {
            unsafe { Component::deserialize(engine, bytes) }.map_err(|error| format!("{error:#}"))
        },
        || Component::from_binary(engine, wasm_bytes).map_err(|error| format!("{error:#}")),
        |component| component.serialize().map_err(|error| error.to_string()),
    )?;
    let handle = next_handle();
    components()
        .lock()
        .unwrap()
        .insert(handle, Arc::new(ComponentEntry { component }));
    Ok(handle)
}

pub fn compile_component_for_api(bytes: &[u8], api_level: u32) -> Result<i64, String> {
    match api_level {
        1..=4 => compile_component(bytes),
        _ => Err(format!("unsupported plugin API level {api_level}")),
    }
}

pub fn precompile_component(wasm_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let engine = get_engine();
    engine
        .precompile_component(wasm_bytes)
        .map_err(|e| format!("{e:#}"))
}

pub fn deserialize_component(cwasm_bytes: &[u8]) -> Result<i64, String> {
    let engine = get_engine();
    let component =
        unsafe { Component::deserialize(engine, cwasm_bytes) }.map_err(|e| format!("{e:#}"))?;
    let handle = next_handle();
    components().lock().unwrap().insert(
        handle,
        Arc::new(ComponentEntry {
            component: Arc::new(component),
        }),
    );
    Ok(handle)
}

fn component_executor() -> &'static tokio::runtime::Handle {
    static EXECUTOR: OnceLock<tokio::runtime::Handle> = OnceLock::new();
    EXECUTOR.get_or_init(|| {
        let (ready, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("abp-components".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to initialize component executor");
                let _ = ready.send(runtime.handle().clone());
                runtime.block_on(std::future::pending::<()>());
            })
            .expect("failed to start component executor");
        receiver.recv().expect("component executor failed to start")
    })
}

pub fn instantiate_component(
    component_handle: i64,
    preopens: &[(String, String)],
) -> Result<i64, String> {
    let comp_entry = {
        let guard = components().lock().unwrap();
        guard
            .get(&component_handle)
            .cloned()
            .ok_or("invalid component handle")?
    };

    let engine = get_engine();
    let rendered_ui = Arc::new(Mutex::new(None));
    let next_node_id = Arc::new(std::sync::atomic::AtomicU64::new(1));

    let (commands, receiver) = tokio::sync::mpsc::unbounded_channel();
    let bridge = Arc::new(ComponentBridge::default());
    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder.inherit_stdout().inherit_stderr();
    for (guest, host) in preopens {
        wasi_builder
            .preopened_dir(
                host,
                guest,
                if guest == "/plugin" {
                    wasmtime_wasi::FsPerms::ReadOnly
                } else {
                    wasmtime_wasi::FsPerms::ReadWrite
                },
            )
            .map_err(|e| format!("preopen {guest} failed: {e:#}"))?;
    }
    let wasi_ctx = wasi_builder.build();
    let http_ctx = WasiHttpCtx::new();
    let table = ResourceTable::new();

    let handle = next_handle();
    let (released, release_complete) = std::sync::mpsc::channel();
    let mut store = Store::new(
        engine,
        ComponentInstanceContext {
            bridge: bridge.clone(),
            commands: commands.clone(),
            timers: HashMap::new(),
            next_timer_id: 0,
            wasi_ctx,
            http_ctx,

            http_hooks: crate::component_http::HttpHooks(bridge.clone()),
            table,
            limits: StoreLimitsBuilder::new()
                .memory_size(128 * 1024 * 1024)
                .build(),
            rendered_ui: rendered_ui.clone(),
            next_node_id,
            _lifetime: StoreLifetime::new(),
            _component: Some(comp_entry.component.clone()),
        },
    );

    store.limiter(|ctx| &mut ctx.limits);
    store.epoch_deadline_async_yield_and_update(1);
    store.set_epoch_deadline(1);
    let mut linker = Linker::new(engine);

    {
        wasmtime_wasi::p2::add_to_linker_async(&mut linker).map_err(|e| e.to_string())?;
        wasmtime_wasi_http::p2::add_only_http_to_linker_async(&mut linker)
            .map_err(|e| e.to_string())?;
        wasmtime_wasi::p3::add_to_linker(&mut linker).map_err(|e| e.to_string())?;
        wasmtime_wasi_http::p3::add_to_linker(&mut linker).map_err(|e| e.to_string())?;
    }
    crate::ab_host::register_ab_host_apis(&mut linker, &comp_entry.component)
        .map_err(|e| e.to_string())?;

    // Android threads can have different PAC keys. A suspended fiber must be
    // resumed and destroyed on its original thread, including during instantiation.
    let executor = component_executor();
    let (ready, initialized) = std::sync::mpsc::sync_channel(1);
    let bridge_for_task = bridge.clone();
    let rendered_for_task = rendered_ui.clone();
    let driver = ComponentDriver {
        future: Box::pin(async move {
            let instance = match linker
                .instantiate_async(&mut store, &comp_entry.component)
                .await
            {
                Ok(instance) => instance,
                Err(error) => {
                    let error = format!("{error:#}");
                    let _ = ready.send(Err(error.clone()));
                    return Err(error);
                }
            };
            if ready.send(Ok(())).is_err() {
                return Ok(());
            }
            drive_component(instance, store, receiver, rendered_for_task).await
        }),
        _released: ReleaseDriver(released),
    };
    let task = executor.spawn(async move {
        let mut driver =
            wasmtime_wasi::runtime::AbortOnDropJoinHandle::from(executor.spawn(driver));
        let outcome = match (&mut *driver).await {
            Ok(result) => result,
            Err(error) => Err(format!("Plugin driver stopped: {error}")),
        };
        if let Err(error) = outcome {
            bridge_for_task.emit(serde_json::json!({"type": "error", "error": error}));
        }
    });
    if let Err(error) = initialized
        .recv()
        .map_err(|_| "Component executor stopped during instantiation".to_string())
        .and_then(|result| result)
    {
        task.abort();
        let _ = release_complete.recv();
        return Err(error);
    }
    instances().lock().unwrap().insert(
        handle,
        ComponentInstanceEntry {
            commands,
            task,
            bridge,
            rendered_ui,
            released: release_complete,
        },
    );
    Ok(handle)
}

pub fn drop_component_instance(handle: i64) -> bool {
    let entry = instances().lock().unwrap().remove(&handle);
    if let Some(entry) = entry {
        entry.bridge.requests.lock().unwrap().clear();
        entry.task.abort();
        // Wait until Store destruction runs on its owning executor thread.
        let _ = entry.released.recv();
        true
    } else {
        false
    }
}

pub fn drop_component(handle: i64) -> bool {
    components().lock().unwrap().remove(&handle).is_some()
}

struct DrainFuture<T>(std::marker::PhantomData<T>);

impl<T> wasmtime::component::FutureConsumer<ComponentInstanceContext> for DrainFuture<T>
where
    T: wasmtime::component::Lift + Send + Unpin + 'static,
{
    type Item = T;

    fn poll_consume(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        store: StoreContextMut<ComponentInstanceContext>,
        mut source: wasmtime::component::Source<'_, T>,
        finish: bool,
    ) -> std::task::Poll<crate::runtime_error::Result<()>> {
        let mut value = None;
        source.read(store, &mut value)?;
        if value.is_some() || finish {
            std::task::Poll::Ready(Ok(()))
        } else {
            std::task::Poll::Pending
        }
    }
}

fn drain_future<T>(
    accessor: &wasmtime::component::Accessor<ComponentInstanceContext>,

    value: &wasmtime::component::Val,
) -> crate::runtime_error::Result<()>
where
    T: wasmtime::component::Lift + Send + Unpin + 'static,
{
    accessor.with(|mut access| {
        let future = match value {
            wasmtime::component::Val::Future(future) => {
                future.clone().try_into_future_reader::<T>()?
            }
            _ => crate::runtime_error::bail!("expected future"),
        };
        // Legacy exports return a future handle. Its producer may depend on a later
        // event, so attach the consumer without waiting for the payload here.
        future.pipe(&mut access, DrainFuture(std::marker::PhantomData::<T>))?;

        Ok(())
    })
}

async fn invoke_export(
    accessor: &wasmtime::component::Accessor<ComponentInstanceContext>,
    func: wasmtime::component::Func,
    args: Vec<wasmtime::component::Val>,
) -> Result<Option<serde_json::Value>, String> {
    let params = accessor.with(|mut access| {
        func.ty(&mut access)
            .params()
            .map(|(name, ty)| (name.to_owned(), ty))
            .collect::<Vec<_>>()
    });
    let args = params
        .iter()
        .zip(args)
        .map(|((_, ty), value)| {
            if let wasmtime::component::Val::String(json) = &value {
                if !matches!(ty, wasmtime::component::Type::String) {
                    return crate::component_host::json_to_val(
                        ty,
                        &serde_json::from_str(json).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string());
                }
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, String>>()?;

    let result_types =
        accessor.with(|mut access| func.ty(&mut access).results().collect::<Vec<_>>());
    let mut results = vec![wasmtime::component::Val::Bool(false); result_types.len()];
    func.call_concurrent(accessor, &args, &mut results)
        .await
        .map_err(|e| format!("{e:#}"))?;
    for (ty, value) in result_types.iter().zip(&results) {
        if let wasmtime::component::Type::Future(future) = ty {
            let result = match future.ty() {
                None => drain_future::<()>(accessor, value),
                Some(wasmtime::component::Type::String) => drain_future::<String>(accessor, value),
                Some(wasmtime::component::Type::Bool) => drain_future::<bool>(accessor, value),
                other => return Err(format!("unsupported plugin future result: {other:?}")),
            };
            result.map_err(|e| format!("{e:#}"))?;
        }
    }
    match results.first() {
        Some(value)
            if !matches!(
                result_types.first(),
                Some(wasmtime::component::Type::Future(_))
            ) =>
        {
            crate::component_host::val_to_json(value)
                .map(Some)
                .map_err(|e| e.to_string())
        }
        _ => Ok(None),
    }
}

async fn drive_component(
    mut instance: Instance,
    mut store: Store<ComponentInstanceContext>,
    mut receiver: tokio::sync::mpsc::UnboundedReceiver<ComponentCommand>,
    rendered_ui: Arc<Mutex<Option<String>>>,
) -> Result<(), String> {
    let mut exports = HashMap::new();
    for (name, interfaces) in [
        (
            "on-load",
            vec![
                "astrobox:psys-plugin-v4/lifecycle",
                "astrobox:psys-plugin/lifecycle",
            ],
        ),
        (
            "on-card-render",
            vec![
                "astrobox:psys-plugin-v4/event",
                "astrobox:psys-plugin/event-v3",
                "astrobox:psys-plugin/event",
            ],
        ),
        (
            "on-ui-render",
            vec![
                "astrobox:psys-plugin-v4/event",
                "astrobox:psys-plugin/event-v3",
                "astrobox:psys-plugin/event",
            ],
        ),
        ("on-ui-event-v3", vec!["astrobox:psys-plugin/event-v3"]),
        (
            "on-ui-event",
            vec![
                "astrobox:psys-plugin-v4/event",
                "astrobox:psys-plugin/event-v3",
                "astrobox:psys-plugin/event",
            ],
        ),
        (
            "on-event",
            vec![
                "astrobox:psys-plugin-v4/event",
                "astrobox:psys-plugin/event-v3",
                "astrobox:psys-plugin/event",
            ],
        ),
        ("handle", vec!["astrobox:psys-plugin-v4/http"]),
        ("on-message", vec!["astrobox:psys-plugin-v4/canopusconnect"]),
    ] {
        if let Some(func) = get_component_func(&mut instance, &mut store, &interfaces, name) {
            exports.insert(name.to_string(), func);
        }
    }
    if let Some(func) = exports.remove("on-ui-event-v3") {
        exports.insert("on-ui-event".to_string(), func);
    }
    let bridge = store.data().bridge.clone();

    store.run_concurrent(async |accessor| {
        let mut calls: Vec<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>>> = Vec::new();
        loop {
            let command = std::future::poll_fn(|cx| {
                let mut index = 0;
                while index < calls.len() {
                    if calls[index].as_mut().poll(cx).is_ready() {
                        drop(calls.swap_remove(index));
                    } else { index += 1; }
                }
                receiver.poll_recv(cx)
            }).await;
                    let Some(ComponentCommand::Call { id, name, args, reply, completed_timer }) = command else { break; };
                    if let Some(timer) = completed_timer {
                        accessor.with(|mut access| { access.get().timers.remove(&timer); });
                    }
                    if name == "has-export" {
                        let present = matches!(args.first(), Some(wasmtime::component::Val::String(name)) if exports.contains_key(name));
                        if let Some(reply) = reply {
                            let _ = reply.send(Ok(present.to_string()));
                        } else {
                            bridge.emit(serde_json::json!({"type": "result", "id": id, "value": present.to_string()}));
                        }
                        continue;
                    }
                    let func = exports.get(&name).copied();
                    let bridge = bridge.clone();
                    let rendered_ui = rendered_ui.clone();
                    calls.push(Box::pin(async move {
                        let result = match func {
                            Some(func) => invoke_export(accessor, func, args).await,
                            None if matches!(name.as_str(), "on-load" | "on-ui-render") => Ok(None),
                            None => Err(format!("Plugin does not export {name}")),
                        }.map(|value| match value {
                            Some(serde_json::Value::String(text)) => text,
                            Some(value) => value.to_string(),
                            None => rendered_ui.lock().unwrap().clone().unwrap_or_default(),
                        });
                        if let Some(reply) = reply {
                            let _ = reply.send(result);
                        } else if id != 0 {
                            match result {
                                Ok(ui) => bridge.emit(serde_json::json!({"type": "result", "id": id, "value": ui})),
                                Err(error) => bridge.emit(serde_json::json!({"type": "result", "id": id, "error": error})),
                            }
                        } else if let Err(error) = result {
                            bridge.emit(serde_json::json!({"type": "error", "error": error}));
                        }
                    }));
        }
    }).await.map_err(|e| format!("{e:#}"))
}

fn send_call(
    handle: i64,
    id: u64,
    name: String,
    args: Vec<wasmtime::component::Val>,
    reply: Option<std::sync::mpsc::Sender<Result<String, String>>>,
) -> Result<(), String> {
    let guard = instances().lock().unwrap();
    let entry = guard
        .get(&handle)
        .ok_or("invalid component instance handle")?;
    entry
        .commands
        .send(ComponentCommand::Call {
            id,
            name,
            args,
            reply,
            completed_timer: None,
        })
        .map_err(|_| "Plugin driver stopped".to_string())
}

fn call(handle: i64, name: &str, args: Vec<wasmtime::component::Val>) -> Result<String, String> {
    let (send, receive) = std::sync::mpsc::channel();
    send_call(handle, 0, name.to_string(), args, Some(send))?;
    receive
        .recv()
        .map_err(|_| "Plugin driver stopped".to_string())?
}

pub fn call_component_on_load(handle: i64) -> Result<(), String> {
    call(handle, "on-load", vec![]).map(|_| ())
}
pub fn call_component_on_ui_render(handle: i64, element_id: &str) -> Result<String, String> {
    call(
        handle,
        "on-ui-render",
        vec![wasmtime::component::Val::String(element_id.to_string())],
    )
}
pub fn call_component_on_ui_event(
    handle: i64,
    event_id: &str,
    event: &str,
    payload: &str,
) -> Result<String, String> {
    call(
        handle,
        "on-ui-event",
        vec![
            wasmtime::component::Val::String(event_id.to_string()),
            wasmtime::component::Val::Enum(event.to_string()),
            wasmtime::component::Val::String(payload.to_string()),
        ],
    )
}
pub fn call_component_on_event(handle: i64, event: &str, payload: &str) -> Result<String, String> {
    call(
        handle,
        "on-event",
        vec![
            wasmtime::component::Val::Enum(event.to_string()),
            wasmtime::component::Val::String(payload.to_string()),
        ],
    )
}

pub fn submit_component_call(handle: i64, json: &str) -> Result<(), String> {
    let request: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let id = request["id"].as_u64().ok_or("Missing call id")?;
    let name = request["name"].as_str().ok_or("Missing export name")?;
    let args = request["args"]
        .as_array()
        .ok_or("Missing arguments")?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let text = value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string());
            Ok(
                if (name == "on-ui-event" && index == 1) || (name == "on-event" && index == 0) {
                    wasmtime::component::Val::Enum(text)
                } else {
                    wasmtime::component::Val::String(text)
                },
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    send_call(handle, id, name.to_string(), args, None)
}

pub fn poll_component_events(handle: i64) -> Result<String, String> {
    let guard = instances().lock().unwrap();
    let entry = guard
        .get(&handle)
        .ok_or("invalid component instance handle")?;
    let events = std::mem::take(&mut *entry.bridge.events.lock().unwrap());
    serde_json::to_string(&events).map_err(|e| e.to_string())
}

pub fn respond_component_host(handle: i64, json: &str) -> Result<(), String> {
    let response: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let guard = instances().lock().unwrap();
    let entry = guard
        .get(&handle)
        .ok_or("invalid component instance handle")?;
    let id = response["id"].as_u64().ok_or("Missing request id")?;
    let send = entry
        .bridge
        .requests
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or("Unknown host request")?;
    let value = if let Some(error) = response.get("error") {
        Err(error.to_string())
    } else {
        Ok(response["value"].clone())
    };
    send.send(value)
        .map_err(|_| "Host request was cancelled".to_string())
}

pub fn get_component_rendered_ui(handle: i64) -> Option<String> {
    let guard = instances().lock().unwrap();
    let value = guard.get(&handle)?.rendered_ui.lock().unwrap().clone();
    value
}

#[cfg(test)]
#[path = "../tests/support/component_runtime.rs"]
mod tests;

pub fn diagnostics() -> serde_json::Value {
    let compiled = components().lock().unwrap();
    let running = instances().lock().unwrap();
    serde_json::json!({
        "engineInitialized": ENGINE.get().is_some(),
        "cache": COMPILED.get_or_init(Default::default).diagnostics(),
        "components": compiled.len(), "instances": running.len(),
        "stores": LIVE_STORES.load(std::sync::atomic::Ordering::Relaxed),
        "pendingHostRequests": running.values().map(|entry| entry.bridge.requests.lock().unwrap().len()).sum::<usize>(),
        "queuedEvents": running.values().map(|entry| entry.bridge.events.lock().unwrap().len()).sum::<usize>(),
        "renderedUiBytes": running.values().map(|entry| entry.rendered_ui.lock().unwrap().as_ref().map_or(0, String::len)).sum::<usize>(),
    })
}

pub fn configure_cache(directory: &str) -> Result<(), String> {
    crate::compiled_cache::configure(directory)
}
