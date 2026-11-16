use super::*;

#[test]
fn rendered_elements_release_transferred_resources() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let engine = get_engine();
        let component =
            Component::from_binary(engine, include_bytes!("../fixtures/ui_ownership.wasm"))
                .unwrap();
        let (commands, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut store = Store::new(
            engine,
            ComponentInstanceContext {
                bridge: Arc::new(ComponentBridge::default()),
                commands,
                timers: HashMap::new(),
                next_timer_id: 0,
                wasi_ctx: WasiCtxBuilder::new().build(),
                http_ctx: WasiHttpCtx::new(),

                http_hooks: crate::component_http::HttpHooks(Arc::new(ComponentBridge::default())),
                table: ResourceTable::new(),
                limits: StoreLimitsBuilder::new().build(),
                rendered_ui: Arc::new(Mutex::new(None)),
                next_node_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
                _lifetime: StoreLifetime::new(),
                _component: None,
            },
        );
        store.epoch_deadline_async_yield_and_update(1);
        store.set_epoch_deadline(1);
        let mut linker = Linker::new(engine);
        crate::ab_host::register_ab_host_apis(&mut linker, &component).unwrap();
        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .unwrap();
        let function = instance.get_func(&mut store, "on-load").unwrap();
        for _ in 0..100 {
            store
                .run_concurrent(async |accessor| {
                    function.call_concurrent(accessor, &[], &mut []).await
                })
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                store
                    .data_mut()
                    .table
                    .iter_mut()
                    .filter(|entry| {
                        let value = &entry.1;
                        value.is::<crate::ab_host::RealElement>()
                    })
                    .count(),
                0,
                "render and child must consume transferred element handles"
            );
            let ui = store.data().rendered_ui.lock().unwrap().clone().unwrap();
            assert!(
                ui.contains("Comic heading"),
                "child content must survive transfer"
            );
            store.data().bridge.events.lock().unwrap().clear();
        }
    });
}

#[test]
fn http_uses_application_transport_and_releases_response() {
    use http_body_util::{BodyExt, Full};
    use hyper::body::Bytes;
    use wasmtime_wasi_http::WasiHttpHooks;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
            let bridge = Arc::new(ComponentBridge::default());
            let serving = bridge.clone();
            let responder = tokio::spawn(async move {
                let mut reads = 0;
                loop {
                    let events = std::mem::take(&mut *serving.events.lock().unwrap());
                    for event in events {
                        let value = match event["method"].as_str().unwrap() {
                            "network.open" => {
                                let request = &event["args"][0];
                                assert_eq!(request["url"], "https://example.test/upload");
                                assert_eq!(request["method"], "POST");
                                assert_eq!(request["body"], serde_json::json!([1, 2, 3]));
                                assert_eq!(request["connectTimeout"], 600_000);
                                assert_eq!(request["firstByteTimeout"], 25);
                                assert_eq!(request["betweenBytesTimeout"], 600_000);
                                serde_json::json!({"status":418,"headers":[{"name":"x-test","value":"response"}]})
                            }
                            "network.read" => {
                                reads += 1;
                                match reads {
                                    1 => serde_json::json!([4, 5]),
                                    2 => serde_json::json!([6]),
                                    3 => serde_json::Value::Null,
                                    _ => panic!("response read after EOF"),
                                }
                            }
                            "network.close" => return reads,
                            method => panic!("unexpected HTTP method {method}"),
                        };
                        let sender = serving.requests.lock().unwrap().remove(&event["id"].as_u64().unwrap()).unwrap();
                        sender.send(Ok(value)).unwrap();
                    }
                    tokio::task::yield_now().await;
                }
            });
            let body = Full::new(Bytes::from_static(&[1, 2, 3]))
                .map_err(|error| match error {}).boxed_unsync();
            let request = hyper::Request::builder().method("POST")
                .uri("https://example.test/upload").body(body).unwrap();
            let mut hooks = crate::component_http::HttpHooks(bridge.clone());
            let options = wasmtime_wasi_http::RequestOptions {
                first_byte_timeout: Some(std::time::Duration::from_millis(25)),
                ..Default::default()
            };
            let (response, io) = Box::into_pin(hooks.send_request(request, Some(options), Box::new(async { Ok(()) }))).await.unwrap();
            assert_eq!(response.status(), 418);
            assert_eq!(response.headers()["x-test"], "response");
            Box::into_pin(io).await.unwrap();
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            assert_eq!(bytes.as_ref(), &[4, 5, 6]);
            let reads = tokio::time::timeout(std::time::Duration::from_secs(2), responder).await.unwrap().unwrap();
            assert_eq!(reads, 3);
            assert!(bridge.requests.lock().unwrap().is_empty());
        });
}

#[test]
fn cancelling_http_clears_pending_host_request() {
    use http_body_util::{BodyExt, Empty};
    use hyper::body::Bytes;
    use wasmtime_wasi_http::WasiHttpHooks;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let bridge = Arc::new(ComponentBridge::default());
        let body = Empty::<Bytes>::new()
            .map_err(|error| match error {})
            .boxed_unsync();
        let request = hyper::Request::builder()
            .uri("https://example.test/cancel")
            .body(body)
            .unwrap();
        let mut hooks = crate::component_http::HttpHooks(bridge.clone());
        let mut request =
            Box::into_pin(hooks.send_request(request, None, Box::new(async { Ok(()) })));
        std::future::poll_fn(|cx| {
            assert!(request.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        assert_eq!(bridge.requests.lock().unwrap().len(), 1);
        drop(request);
        assert!(bridge.requests.lock().unwrap().is_empty());
        let events = bridge.events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["method"], "network.open");
        assert_eq!(events[1]["method"], "network.close");
        assert_eq!(events[0]["args"][0]["id"], events[1]["args"][0]);
    });
}
