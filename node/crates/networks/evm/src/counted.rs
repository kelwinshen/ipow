//! The endpoint's calls, counted by network (`ipow_protocol_core::calls`):
//! a layer under the provider sees every request it sends, a batch as its
//! calls, each by its method.

use std::sync::Arc;
use std::task::{Context, Poll};

use alloy::rpc::json_rpc::{RequestPacket, ResponsePacket};
use alloy::transports::TransportError;

#[derive(Clone)]
pub struct CountLayer(pub Arc<str>);

impl<S> tower::Layer<S> for CountLayer {
    type Service = Counted<S>;
    fn layer(&self, inner: S) -> Counted<S> {
        Counted { inner, network: self.0.clone() }
    }
}

#[derive(Clone)]
pub struct Counted<S> {
    inner: S,
    network: Arc<str>,
}

impl<S> tower::Service<RequestPacket> for Counted<S>
where
    S: tower::Service<RequestPacket, Response = ResponsePacket, Error = TransportError>,
{
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: RequestPacket) -> Self::Future {
        match &req {
            RequestPacket::Single(r) => ipow_protocol_core::calls::count(&self.network, r.method(), 1),
            RequestPacket::Batch(b) => {
                for r in b {
                    ipow_protocol_core::calls::count(&self.network, r.method(), 1);
                }
            }
        }
        self.inner.call(req)
    }
}
