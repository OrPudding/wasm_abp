use wasm_abp::component::{
    call_component_on_load, compile_component, compile_component_for_api, drop_component,
    drop_component_instance, instantiate_component,
};

#[test]
fn legacy_host_future_is_returned_and_drained() {
    let component = compile_component(include_bytes!("fixtures/legacy_future.wasm")).unwrap();
    let instance = instantiate_component(component, &[]).unwrap();
    let outcome = call_component_on_load(instance);
    drop_component_instance(instance);
    drop_component(component);
    outcome.unwrap();
}

#[test]
fn account_placeholder_accepts_historical_profile_shapes() {
    for bytes in [
        include_bytes!("fixtures/account_string.wasm").as_slice(),
        include_bytes!("fixtures/account_optional.wasm").as_slice(),
    ] {
        let component = compile_component(bytes).unwrap();
        let instance = instantiate_component(component, &[]).unwrap();
        drop_component_instance(instance);
        drop_component(component);
    }
}

#[test]
fn legacy_ui_render_size_import_links_as_future() {
    let component = compile_component(include_bytes!("fixtures/ui_v3_size.wasm")).unwrap();
    let instance = instantiate_component(component, &[]).unwrap();
    drop_component_instance(instance);
    drop_component(component);
}

#[test]
fn browser_import_accepts_string_and_record_arguments() {
    for bytes in [
        include_bytes!("fixtures/browser_string.wasm").as_slice(),
        include_bytes!("fixtures/browser_record.wasm").as_slice(),
    ] {
        let component = compile_component(bytes).unwrap();
        let instance = instantiate_component(component, &[]).unwrap();
        drop_component_instance(instance);
        drop_component(component);
    }
}

#[test]
fn legacy_unit_future_completes() {
    let component = compile_component(include_bytes!("fixtures/unit_future.wasm")).unwrap();
    let instance = instantiate_component(component, &[]).unwrap();
    let result = call_component_on_load(instance);
    drop_component_instance(instance);
    drop_component(component);
    result.unwrap();
}

#[test]
fn v3_ui_event_export_is_invoked_and_future_drained() {
    let component = compile_component(include_bytes!("fixtures/event_v3.wasm")).unwrap();
    let instance = instantiate_component(component, &[]).unwrap();
    let result =
        wasm_abp::component::call_component_on_ui_event(instance, "callback", "CHANGE", "value");
    let invalid =
        wasm_abp::component::call_component_on_ui_event(instance, "callback", "change", "value");
    drop_component_instance(instance);
    drop_component(component);
    result.unwrap();
    assert!(
        invalid.is_err(),
        "the v3 export must be invoked and validate the enum"
    );
}

#[test]
fn ui_notification_fixture_renders() {
    let component = compile_component(include_bytes!("fixtures/ui_notification.wasm")).unwrap();
    let instance = instantiate_component(component, &[]).unwrap();
    let result = call_component_on_load(instance);
    let ui = wasm_abp::component::get_component_rendered_ui(instance);
    drop_component_instance(instance);
    drop_component(component);
    result.unwrap();
    assert!(ui.unwrap().contains("Background"));
}

#[test]
fn pending_legacy_ui_future_does_not_block_render_or_next_event() {
    let component = compile_component(include_bytes!("fixtures/pending_ui_future.wasm")).unwrap();
    let instance = instantiate_component(component, &[]).unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| {
            wasm_abp::component::call_component_on_ui_render(instance, "root")?;
            wasm_abp::component::call_component_on_ui_render(instance, "root")
        })();
        let _ = sender.send(result);
    });
    let result = receiver
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("render must return even while the legacy future is pending");
    drop_component_instance(instance);
    drop_component(component);
    result.unwrap();
}

#[test]
fn element_size_sets_font_size_without_constraining_layout() {
    let component = compile_component(include_bytes!("fixtures/text_size.wasm")).unwrap();
    let instance = instantiate_component(component, &[]).unwrap();
    let result = call_component_on_load(instance);
    let ui = wasm_abp::component::get_component_rendered_ui(instance).unwrap();
    drop_component_instance(instance);
    drop_component(component);
    result.unwrap();
    let ui: serde_json::Value = serde_json::from_str(&ui).unwrap();
    let styles = &ui["root"]["styles"];
    assert_eq!(styles["font_size"], 15);
    assert!(styles.get("width").is_none());
    assert!(styles.get("height").is_none());
}

#[test]
fn synchronous_lowering_can_wait_for_the_application_host_without_blocking_submit() {
    use wasm_abp::component::{
        poll_component_events, respond_component_host, submit_component_call,
    };
    let component = compile_component(include_bytes!("fixtures/sync_host.wasm")).unwrap();
    let instance = instantiate_component(component, &[]).unwrap();
    submit_component_call(instance, r#"{"id":1,"name":"on-load","args":[]}"#).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut completed = false;
    while std::time::Instant::now() < deadline && !completed {
        let events: serde_json::Value =
            serde_json::from_str(&poll_component_events(instance).unwrap()).unwrap();
        for event in events.as_array().unwrap() {
            if event["type"] == "host" {
                assert_eq!(event["method"], "os.timezone-offset-minutes");
                respond_component_host(
                    instance,
                    &serde_json::json!({"id":event["id"], "value":480}).to_string(),
                )
                .unwrap();
            } else if event["type"] == "result" {
                assert!(event.get("error").is_none(), "{event}");
                completed = true;
            } else {
                panic!("Unexpected event {event}");
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    drop_component_instance(instance);
    drop_component(component);
    assert!(
        completed,
        "synchronous guest lowering must resume after the host reply"
    );
}

#[test]
fn component_survives_repeated_host_suspensions() {
    use std::time::{Duration, Instant};
    use wasm_abp::component::{
        poll_component_events, respond_component_host, submit_component_call,
    };

    let component = compile_component(include_bytes!("fixtures/sync_host.wasm")).unwrap();
    let instance = instantiate_component(component, &[]).unwrap();
    for id in 1..=100 {
        submit_component_call(
            instance,
            &serde_json::json!({"id": id, "name": "on-load", "args": []}).to_string(),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut completed = false;
        while Instant::now() < deadline && !completed {
            let events: serde_json::Value =
                serde_json::from_str(&poll_component_events(instance).unwrap()).unwrap();
            for event in events.as_array().unwrap() {
                match event["type"].as_str().unwrap() {
                    "host" => {
                        // Allow the guest fiber to suspend before another thread wakes it.
                        std::thread::sleep(Duration::from_millis(2));
                        respond_component_host(
                            instance,
                            &serde_json::json!({"id": event["id"], "value": 480}).to_string(),
                        )
                        .unwrap();
                    }
                    "result" => {
                        assert_eq!(event["id"], id);
                        assert!(event.get("error").is_none(), "{event}");
                        completed = true;
                    }
                    _ => panic!("Unexpected component event {event}"),
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(completed, "Host suspension {id} must resume and finish");
    }
    drop_component_instance(instance);
    drop_component(component);
}

#[test]
fn stopping_a_suspended_component_keeps_the_executor_usable() {
    use std::time::{Duration, Instant};
    use wasm_abp::component::poll_component_events;

    let component = compile_component(include_bytes!("fixtures/sync_host.wasm")).unwrap();
    let instance = instantiate_component(component, &[]).unwrap();
    let (finished, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = finished.send(call_component_on_load(instance));
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let events: serde_json::Value =
            serde_json::from_str(&poll_component_events(instance).unwrap()).unwrap();
        if events
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["type"] == "host")
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Guest must reach the suspended Host call"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    drop_component_instance(instance);
    assert!(completion
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .is_err());
    drop_component(component);

    let component = compile_component(include_bytes!("fixtures/ui_notification.wasm")).unwrap();
    let instance = instantiate_component(component, &[]).unwrap();
    call_component_on_load(instance).unwrap();
    assert!(wasm_abp::component::get_component_rendered_ui(instance)
        .unwrap()
        .contains("Background"));
    drop_component_instance(instance);
    drop_component(component);
}

#[test]
fn wasi_cli_exit_with_code_links_and_reports_guest_exit() {
    let component = compile_component(include_bytes!("fixtures/wasi_exit_code.wasm")).unwrap();
    let instance = instantiate_component(component, &[]).unwrap();
    let result = call_component_on_load(instance);
    result.expect_err("guest exit must terminate its call");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let events = loop {
        let events = wasm_abp::component::poll_component_events(instance).unwrap();
        if events.contains("exit status 7") || std::time::Instant::now() >= deadline {
            break events;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    drop_component_instance(instance);
    drop_component(component);
    assert!(events.contains("exit status 7"), "{events}");
}

#[test]
fn supported_api_levels_share_wasi_p3_and_reject_unknown_levels() {
    let bytes = include_bytes!("fixtures/wasi_p3_environment.wasm");
    for api in 1..=4 {
        let component = compile_component_for_api(bytes, api).unwrap();
        let instance = instantiate_component(component, &[]).unwrap();
        assert!(drop_component_instance(instance));
        assert!(!drop_component_instance(instance));
        assert!(drop_component(component));
    }
    assert!(compile_component_for_api(bytes, 5).is_err());
}

#[test]
fn close_releases_compiled_code_before_returning() {
    const CHILD: &str = "ORONBOX_DRIVER_RELEASE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "close_releases_compiled_code_before_returning"])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    for _ in 0..30 {
        let component = compile_component(include_bytes!("fixtures/ui_notification.wasm")).unwrap();
        let instance = instantiate_component(component, &[]).unwrap();
        assert!(drop_component(component));
        call_component_on_load(instance).unwrap();
        assert!(drop_component_instance(instance));
        let backend = wasm_abp::component::diagnostics();
        assert_eq!(backend["components"], 0);
        assert_eq!(backend["instances"], 0);
        assert_eq!(backend["stores"], 0);
        assert_eq!(backend["pendingHostRequests"], 0);
        assert_eq!(backend["queuedEvents"], 0);
        assert_eq!(backend["renderedUiBytes"], 0);
        assert_eq!(backend["cache"]["activeCompiledObjects"], 0);
    }
}

#[test]
fn guest_sync_wait_requires_the_patch_and_can_be_cancelled() {
    use wasm_abp::component::{poll_component_events, submit_component_call};
    for bytes in [
        include_bytes!("fixtures/legacy_sync_wait.wasm").as_slice(),
        include_bytes!("fixtures/nested_sync_wait.wasm").as_slice(),
    ] {
        let mut config = wasmtime::Config::new();
        config
            .wasm_component_model(true)
            .wasm_component_model_async(true);
        let engine = wasmtime::Engine::new(&config).unwrap();
        let component = wasmtime::component::Component::from_binary(&engine, bytes).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime.block_on(async {
            let mut store = wasmtime::Store::new(&engine, ());
            let linker = wasmtime::component::Linker::new(&engine);
            let instance = linker
                .instantiate_async(&mut store, &component)
                .await
                .unwrap();
            let func = instance.get_func(&mut store, "on-load").unwrap();
            store
                .run_concurrent(async |accessor| func.call_concurrent(accessor, &[], &mut []).await)
                .await
                .and_then(|result| result)
                .unwrap_err()
        });
        assert!(format!("{error:#}").contains("cannot block a synchronous task before returning"));

        let component = compile_component_for_api(bytes, 3).unwrap();
        let instance = instantiate_component(component, &[]).unwrap();
        submit_component_call(instance, r#"{"id":1,"name":"on-load","args":[]}"#).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150));
        // No event can complete an empty waitable set. It must remain pending, without trapping.
        assert_eq!(poll_component_events(instance).unwrap(), "[]");
        assert!(drop_component_instance(instance));
        assert!(drop_component(component));
    }
}

#[test]
fn concurrent_plugins_keep_host_replies_and_failures_isolated() {
    use std::time::{Duration, Instant};
    use wasm_abp::component::{
        poll_component_events, respond_component_host, submit_component_call,
    };
    let component =
        compile_component_for_api(include_bytes!("fixtures/sync_host.wasm"), 3).unwrap();
    let instances: Vec<_> = (0..4)
        .map(|_| instantiate_component(component, &[]).unwrap())
        .collect();
    for &instance in &instances {
        submit_component_call(instance, r#"{"id":1,"name":"on-load","args":[]}"#).unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut requests = std::collections::HashMap::new();
    while requests.len() != instances.len() {
        for &instance in &instances {
            let events: serde_json::Value =
                serde_json::from_str(&poll_component_events(instance).unwrap()).unwrap();
            for event in events.as_array().unwrap() {
                assert_eq!(event["type"], "host", "{event}");
                requests.insert(instance, event["id"].clone());
            }
        }
        assert!(
            Instant::now() < deadline,
            "all plugins must independently reach the host"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    // Keep one instance suspended while the others receive replies in reverse order.
    for (index, &instance) in instances.iter().enumerate().skip(1).rev() {
        let reply = if index == 2 {
            serde_json::json!({"id":requests[&instance],"error":"host request failed"})
        } else {
            serde_json::json!({"id":requests[&instance],"value":480})
        };
        respond_component_host(instance, &reply.to_string()).unwrap();
        loop {
            let events: serde_json::Value =
                serde_json::from_str(&poll_component_events(instance).unwrap()).unwrap();
            if let Some(event) = events
                .as_array()
                .unwrap()
                .iter()
                .find(|event| event["type"] == "result" || event["type"] == "error")
            {
                assert_eq!(event.get("error").is_some(), index == 2, "{event}");
                break;
            }
            assert!(
                Instant::now() < deadline,
                "one suspended plugin must not block another"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    assert_eq!(poll_component_events(instances[0]).unwrap(), "[]");
    for instance in instances {
        assert!(drop_component_instance(instance));
    }
    assert!(drop_component(component));
}
