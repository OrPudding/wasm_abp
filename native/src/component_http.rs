use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use http_body_util::BodyExt;
use serde_json::json;
use wasmtime_wasi_http::{Error, RequestOptions, WasiBody, WasiHttpHooks};

use crate::component::ComponentBridge;
use crate::component_http_body::{RequestLease, ResponseBody};

type IoFuture = Box<dyn Future<Output = Result<(), Error>> + Send>;
type Response = (hyper::Response<WasiBody>, IoFuture);

pub struct HttpHooks(pub Arc<ComponentBridge>);

impl WasiHttpHooks for HttpHooks {
    fn send_request(
        &mut self,
        request: hyper::Request<WasiBody>,
        options: Option<RequestOptions>,
        _response_processing: IoFuture,
    ) -> Box<dyn Future<Output = Result<Response, Error>> + Send> {
        Box::new(send_request(self.0.clone(), request, options))
    }
}

async fn send_request(
    bridge: Arc<ComponentBridge>,
    request: hyper::Request<WasiBody>,
    options: Option<RequestOptions>,
) -> Result<Response, Error> {
    static NEXT_REQUEST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = NEXT_REQUEST.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let lease = RequestLease {
        bridge: bridge.clone(),
        id,
    };
    let config = options.unwrap_or_default();
    let (parts, body) = request.into_parts();
    let bytes = body.collect().await?.to_bytes();
    let response = async {
        let headers = parts.headers.iter().map(|(name, value)| {
            Ok(json!({"name": name.as_str(), "value": value.to_str()?}))
        }).collect::<anyhow::Result<Vec<_>>>()?;
        // Match Wasmtime's default request deadlines when the guest supplies none.
        let default_timeout = Duration::from_secs(600);
        let response = bridge.request("network.open".to_string(), json!([{
            "id": id,
            "url": parts.uri.to_string(),
            "method": parts.method.as_str(),
            "headers": headers,
            "body": bytes.as_ref(),
            "connectTimeout": config.connect_timeout.unwrap_or(default_timeout).as_millis() as u64,
            "firstByteTimeout": config.first_byte_timeout.unwrap_or(default_timeout).as_millis() as u64,
            "betweenBytesTimeout": config.between_bytes_timeout.unwrap_or(default_timeout).as_millis() as u64,
        }])).await.map_err(anyhow::Error::msg)?;
        let status = response["status"].as_u64()
            .ok_or_else(|| anyhow::anyhow!("Missing HTTP status"))?;
        let mut builder = hyper::Response::builder().status(u16::try_from(status)?);
        let headers = response["headers"].as_array()
            .ok_or_else(|| anyhow::anyhow!("Missing HTTP headers"))?;
        for header in headers {
            let name = header["name"].as_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid header name"))?;
            let value = header["value"].as_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid header value"))?;
            builder = builder.header(name, value);
        }
        builder.body(ResponseBody::new(lease).boxed_unsync()).map_err(anyhow::Error::from)
    }.await.map_err(|error| Error::InternalError(Some(error.to_string())))?;
    // Application requests run through the mailbox. The response body drives reads on demand.
    Ok((response, Box::new(async { Ok(()) })))
}
