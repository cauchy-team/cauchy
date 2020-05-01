use std::{error::Error, pin::Pin, sync::Arc, time::Instant};

use futures::{
    channel::mpsc,
    prelude::*,
    task::{Context, Poll},
};
use network::{
    codec::{Status, Transactions},
    Message,
};
use pin_project::pin_project;
use tokio::sync::RwLock;
use tokio_tower::pipeline::Client;
use tower::{buffer::Buffer, Service};

pub type TowerError = tokio_tower::Error<ClientTransport, Message>;

use super::*;

#[pin_project]
pub struct ClientTransport {
    /// Incoming responses
    #[pin]
    sink: mpsc::Sender<Message>,
    /// Outgoing requests
    #[pin]
    stream: mpsc::Receiver<Message>,
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

#[derive(Clone)]
pub struct PeerClient {
    metadata: Arc<Metadata>,
    last_status: Arc<RwLock<Option<Status>>>,
    connection: Buffer<Client<ClientTransport, TowerError, Message>, Message>,
}

const BUFFER_SIZE: usize = 128;

impl PeerClient {
    pub fn new(
        metadata: Arc<Metadata>,
        sink: mpsc::Sender<Message>,
        stream: mpsc::Receiver<Message>,
    ) -> Self {
        let client_transport = ClientTransport { sink, stream };
        Self {
            metadata,
            last_status: Default::default(),
            connection: Buffer::new(Client::new(client_transport), BUFFER_SIZE),
        }
    }
}

impl PeerClient {
    pub fn get_metadata(&self) -> Arc<Metadata> {
        self.metadata.clone()
    }

    pub fn get_socket(&self) -> SocketAddr {
        self.metadata.addr.clone()
    }
}

pub struct UpdateStatus;

pub enum UpdateError {
    UnexpectedResponse,
    Tower(Box<dyn Error + Send + Sync>),
}

impl Service<UpdateStatus> for PeerClient {
    type Response = Status;
    type Error = UpdateError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.connection.poll_ready(cx).map_err(UpdateError::Tower)
    }

    fn call(&mut self, _: UpdateStatus) -> Self::Future {
        let response_fut = self.connection.call(Message::Poll);

        let last_status_inner = self.last_status.clone();
        let fut = async move {
            let response = response_fut.await;
            match response {
                Ok(Message::Status(status)) => {
                    let new_status = status.clone();
                    *last_status_inner.write().await = Some(new_status);
                    Ok(status)
                }
                Ok(_) => Err(UpdateError::UnexpectedResponse),
                Err(err) => Err(UpdateError::Tower(err)),
            }
        };
        Box::pin(fut)
    }
}

pub struct Reconcile(Minisketch);

pub enum ReconcileError {
    UnexpectedResponse,
    Tower(Box<dyn Error + Send + Sync>),
}

impl Service<Reconcile> for PeerClient {
    type Response = Transactions;
    type Error = ReconcileError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.connection
            .poll_ready(cx)
            .map_err(ReconcileError::Tower)
    }

    fn call(&mut self, _: Reconcile) -> Self::Future {
        let response_fut = self.connection.call(Message::Poll);

        let fut = async move {
            let response = response_fut.await;
            match response {
                Ok(Message::ReconcileResponse(txs)) => Ok(txs),
                Ok(_) => Err(ReconcileError::UnexpectedResponse),
                Err(err) => Err(ReconcileError::Tower(err)),
            }
        };
        Box::pin(fut)
    }
}

pub struct GetStatus;
pub struct MissingStatus;

impl Service<GetStatus> for PeerClient {
    type Response = Status;
    type Error = MissingStatus;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

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
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetMetadata) -> Self::Future {
        let metadata = self.metadata.clone();
        let fut = async move { Ok(metadata) };
        Box::pin(fut)
    }
}
