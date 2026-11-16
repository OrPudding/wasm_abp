use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use hyper::body::{Body, Bytes, Frame};
use serde_json::{json, Value};

use wasmtime_wasi_http::Error as ErrorCode;

use crate::component::ComponentBridge;

pub(crate) struct RequestLease {
    pub bridge: Arc<ComponentBridge>,
    pub id: u64,
}

impl Drop for RequestLease {
    fn drop(&mut self) {
        self.bridge.notify("network.close", json!([self.id]));
    }
}

type ReadFuture = Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>;

pub(crate) struct ResponseBody {
    lease: RequestLease,
    pending: Mutex<Option<ReadFuture>>,
    ended: bool,
}

impl Body for ResponseBody {
    type Data = Bytes;
    type Error = ErrorCode;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, ErrorCode>>> {
        if self.ended {
            return Poll::Ready(None);
        }
        let bridge = self.lease.bridge.clone();
        let id = self.lease.id;
        let pending = self.pending.get_mut().unwrap();
        let reading = pending.get_or_insert_with(|| {
            Box::pin(async move {
                bridge
                    .request("network.read".to_string(), json!([id]))
                    .await
            })
        });
        let result = match reading.as_mut().poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(result) => result,
        };
        *pending = None;
        match result {
            Ok(Value::Null) => {
                self.ended = true;
                Poll::Ready(None)
            }
            Ok(value) => match serde_json::from_value::<Vec<u8>>(value) {
                Ok(bytes) => Poll::Ready(Some(Ok(Frame::data(Bytes::from(bytes))))),
                Err(error) => {
                    self.ended = true;
                    Poll::Ready(Some(Err(ErrorCode::InternalError(Some(error.to_string())))))
                }
            },
            Err(error) => {
                self.ended = true;
                Poll::Ready(Some(Err(ErrorCode::InternalError(Some(error)))))
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.ended
    }
}

impl ResponseBody {
    pub fn new(lease: RequestLease) -> Self {
        Self {
            lease,
            pending: Mutex::new(None),
            ended: false,
        }
    }
}
