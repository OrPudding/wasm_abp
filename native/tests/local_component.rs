use std::time::{Duration, Instant};
use wasm_abp::component::*;

fn wait_for_result(instance: i64, id: u64, expected_text: Option<&str>) {
    let host_responses: serde_json::Map<String, serde_json::Value> =
        std::env::var("ABP_COMPONENT_HOST_RESPONSES")
            .map(|value| {
                serde_json::from_str(&value).expect("Host responses must be a JSON object")
            })
            .unwrap_or_default();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut completed = false;
    while Instant::now() < deadline {
        let events: serde_json::Value =
            serde_json::from_str(&poll_component_events(instance).unwrap()).unwrap();
        for event in events.as_array().unwrap() {
            assert!(
                event.get("error").is_none(),
                "component error batch: {events}"
            );
        }
        for event in events.as_array().unwrap() {
            match event["type"].as_str().unwrap() {
                "host" => {
                    let method = event["method"].as_str().unwrap();
                    let value = match method {
                        "device.get-device-list" | "device.get-connected-device-list" => {
                            serde_json::json!([])
                        }
                        "ui.get-render-size" => serde_json::json!({"width": 400, "height": 800}),
                        "os.device-id" => serde_json::json!("test-installation"),
                        "os.appearance" => serde_json::json!("light"),
                        "os.locale" | "os.astrobox-language" => serde_json::json!("zh-CN"),
                        name if name.starts_with("register.") => serde_json::Value::Null,
                        _ => host_responses.get(method).cloned().unwrap_or_else(|| {
                            panic!("Provide an explicit fixture response for {method}")
                        }),
                    };
                    respond_component_host(
                        instance,
                        &serde_json::json!({"id":event["id"],"value":value}).to_string(),
                    )
                    .unwrap_or_else(|error| {
                        panic!("Host response failed: {error}; queued events: {}", events);
                    });
                }
                "result" => {
                    assert!(event.get("error").is_none(), "{event}");
                    if event["id"] == id {
                        completed = true;
                    }
                }
                "error" => panic!("{event}"),
                "ui" => {}
                other => panic!("Unexpected component event {other}"),
            }
        }
        let ui = get_component_rendered_ui(instance);
        if completed
            && expected_text.is_none_or(|text| ui.as_ref().is_some_and(|ui| ui.contains(text)))
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("component call {id} did not complete with expected UI {expected_text:?}");
}

/// Exercises a locally supplied production component without copying its binary into the repository.
#[test]
#[ignore = "requires ABP_COMPONENT_PATH and ABP_COMPONENT_DATA"]
fn local_component_load_and_render() {
    let path = std::env::var("ABP_COMPONENT_PATH").unwrap();
    let data = std::env::var("ABP_COMPONENT_DATA").unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let api_level = std::env::var("ABP_COMPONENT_API_LEVEL")
        .map(|value| value.parse::<u32>().expect("API level must be an integer"))
        .unwrap_or(3);
    let component =
        compile_component_for_api(&bytes, api_level).expect("production component must compile");
    let instance = instantiate_component(
        component,
        &[
            ("/data".into(), data.clone()),
            ("/cache".into(), data.clone()),
            ("/temp".into(), data.clone()),
            (".".into(), data),
        ],
    )
    .expect("production component imports must link");
    let outcome =
        std::panic::catch_unwind(|| {
            submit_component_call(instance, r#"{"id":1,"name":"on-load","args":[]}"#).unwrap();
            wait_for_result(instance, 1, None);
            submit_component_call(
                instance,
                r#"{"id":2,"name":"on-ui-render","args":["root"]}"#,
            )
            .unwrap();
            wait_for_result(instance, 2, Some("root"));
            if let Ok(actions) = std::env::var("ABP_COMPONENT_ACTIONS") {
                let actions: Vec<serde_json::Value> = serde_json::from_str(&actions).unwrap();
                let repeats = std::env::var("ABP_COMPONENT_ACTION_REPEATS")
                    .map(|value| {
                        value
                            .parse::<usize>()
                            .expect("action repeats must be an integer")
                    })
                    .unwrap_or(1);
                for (index, action) in actions
                    .iter()
                    .cycle()
                    .take(actions.len() * repeats)
                    .enumerate()
                {
                    let id = index as u64 + 3;
                    submit_component_call(instance, &serde_json::json!({
                    "id":id, "name":"on-ui-event",
                    "args":[action["callback"], action["event"], action["payload"].to_string()],
                }).to_string()).unwrap();
                    wait_for_result(instance, id, action["text"].as_str());
                }
            }
            if let Ok(path) = std::env::var("ABP_COMPONENT_UI_OUTPUT") {
                std::fs::write(path, get_component_rendered_ui(instance).unwrap()).unwrap();
            }
        });
    drop_component_instance(instance);
    drop_component(component);
    outcome.unwrap();
}
