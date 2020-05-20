use std::{net::SocketAddr, pin::Pin, sync::Arc};

use common::{
    network::{Status, Transactions},
    services::*,
    FutResponse,
};
use futures_channel::mpsc;
use futures_core::{
    stream::Stream,
    task::{Context, Poll},
};
use futures_sink::Sink;
use futures_util::future::AbortHandle;
use network::Message;
use pin_project::pin_project;
use tokio::sync::RwLock;
use tokio_tower::pipeline::Client;
use tower_buffer::Buffer;
use tower_service::Service;
use tracing::info;

use super::*;

pub type TowerError<T> = tokio_tower::Error<T, Message>;

/// Underlying transport for the `PeerClient`. Used to forward messages to a remote peer.
#[pin_project]
pub struct ClientTransport {
    /// Incoming responses.
    #[pin]
    sink: mpsc::Sender<Message>,
    /// Outgoing requests.
    #[pin]
    stream: mpsc::Receiver<Message>,
}

impl ClientTransport {
    /// Construct new  `ClientTransport` from request `Sender` and response `Receiver`.
    pub fn new(
        request_sink: mpsc::Sender<Message>,
        response_stream: mpsc::Receiver<Message>,
    ) -> Self {
        Self {
            sink: request_sink,
            stream: response_stream,
        }
    }
}

impl Stream for ClientTransport {
    type Item = Result<Message, mpsc::SendError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project()
            .stream
            .poll_next(cx)
            .map(|some| some.map(|some| Ok(some)))
    }
}

impl Sink<Message> for ClientTransport {
    type Error = mpsc::SendError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sink.poll_ready(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sink.poll_close(cx)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sink.poll_flush(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        self.project().sink.start_send(item)
    }
}

/// A low-level peer client service constructed from the `ClientTransport`.
///
/// This is wrapped by the high level `PeerClient`.
pub type ClientService =
    Buffer<Client<ClientTransport, TowerError<ClientTransport>, Message>, Message>;

/// A client responsible for communication with a specific peer. It handles request/responses and holds `Metadata` and the latest cached `Status`.
///
/// When dropped the `PeerServer` will stop execution.
#[derive(Clone)]
pub struct PeerClient {
    metadata: Arc<Metadata>,
    last_status: Arc<RwLock<Option<Status>>>,
    client_svc: ClientService,
    terminator: AbortHandle,
}

impl Drop for PeerClient {
    fn drop(&mut self) {
        // Stop server on drop
        self.terminator.abort();
    }
}

impl PeerClient {
    /// Construct a new `PeerClient`.
    pub fn new(
        metadata: Arc<Metadata>,
        last_status: Arc<RwLock<Option<Status>>>,
        client_svc: ClientService,
        terminator: AbortHandle,
    ) -> Self {
        Self {
            metadata,
            client_svc,
            last_status,
            terminator,
        }
    }
}

// TODO: Make this into a service?
impl PeerMetadata for PeerClient {
    /// Get the peers `Metadata`.
    fn get_metadata(&self) -> Arc<Metadata> {
        self.metadata.clone()
    }

    /// Get the peers `SocketAddr`.
    fn get_socket(&self) -> SocketAddr {
        self.metadata.addr.clone()
    }
}

/// An error encountered while calling `PollStatus`.
pub enum PollStatusError {
    /// Peer responded with an unexpected response.
    UnexpectedResponse,
    /// Server error.
    Tower(Box<dyn std::error::Error + Send + Sync>),
}

impl Service<PollStatus> for PeerClient {
    type Response = Status;
    type Error = PollStatusError;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.client_svc
            .poll_ready(cx)
            .map_err(PollStatusError::Tower)
    }

    fn call(&mut self, _: PollStatus) -> Self::Future {
        let response_fut = self.client_svc.call(Message::Poll);

        let last_status_inner = self.last_status.clone();
        let fut = async move {
            let response = response_fut.await;
            match response {
                Ok(Message::Status(status)) => {
                    let new_status = status.clone();
                    *last_status_inner.write().await = Some(new_status);
                    Ok(status)
                }
                Ok(_) => Err(PollStatusError::UnexpectedResponse),
                Err(err) => Err(PollStatusError::Tower(err)),
            }
        };
        info!("polling peer");
        Box::pin(fut)
    }
}

/// An error encountered while calling `Reconcile`.
pub enum ReconcileError {
    /// Peer responded with an unexpected response.
    UnexpectedResponse,
    /// Server error.
    Tower(Box<dyn std::error::Error + Send + Sync>),
}

impl Service<Reconcile> for PeerClient {
    type Response = Transactions;
    type Error = ReconcileError;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.client_svc
            .poll_ready(cx)
            .map_err(ReconcileError::Tower)
    }

    fn call(&mut self, Reconcile(minisketch): Reconcile) -> Self::Future {
        let response_fut = self.client_svc.call(Message::Reconcile(minisketch));

        let fut = async move {
            let response = response_fut.await;
            match response {
                Ok(Message::ReconcileResponse(txs)) => Ok(txs),
                Ok(_) => Err(ReconcileError::UnexpectedResponse),
                Err(err) => Err(ReconcileError::Tower(err)),
            }
        };
        info!("reconciling with peer");
        Box::pin(fut)
    }
}

impl Service<GetStatus> for PeerClient {
    type Response = Status;
    type Error = MissingStatus;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetStatus) -> Self::Future {
        let last_status_inner = self.last_status.clone();
        let fut = async move { last_status_inner.read().await.clone().ok_or(MissingStatus) };
        Box::pin(fut)
    }
}

impl Service<GetMetadata> for PeerClient {
    type Response = Arc<Metadata>;
    type Error = ();
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetMetadata) -> Self::Future {
        let metadata = self.metadata.clone();
        let fut = async move { Ok(metadata) };
        Box::pin(fut)
    }
}
