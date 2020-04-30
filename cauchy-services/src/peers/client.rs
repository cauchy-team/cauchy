use std::{net::SocketAddr, pin::Pin, sync::Arc, time::Instant};

use arena::Minisketch;
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
use tower_service::Service;

pub type TowerError = tokio_tower::Error<ClientTransport, Message>;

use super::*;

#[pin_project]
pub struct ClientTransport {
    #[pin]
    sink: mpsc::Sender<Message>,
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

pub struct PeerClient {
    start_time: Instant,
    addr: SocketAddr,
    last_status: Arc<RwLock<Option<Status>>>,
    connection: Client<ClientTransport, TowerError, Message>,
}

impl PeerClient {
    pub fn new(
        start_time: Instant,
        addr: SocketAddr,
        sink: mpsc::Sender<Message>,
        stream: mpsc::Receiver<Message>,
    ) -> Self {
        let client_transport = ClientTransport { sink, stream };
        Self {
            start_time,
            addr,
            last_status: Default::default(),
            connection: Client::new(client_transport),
        }
    }
}

pub struct UpdateStatus;

pub enum UpdateError {
    UnexpectedResponse,
    Tower(TowerError),
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
    Tower(TowerError),
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
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetStatus) -> Self::Future {
        let last_status_inner = self.last_status.clone();
        let fut = async move { last_status_inner.read().await.clone().ok_or(MissingStatus) };
        Box::pin(fut)
    }
}

pub struct Metadata {
    start_time: Instant,
    addr: SocketAddr,
}

impl Service<GetMetadata> for PeerClient {
    type Response = Metadata;
    type Error = ();
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetMetadata) -> Self::Future {
        let start_time = self.start_time.clone();
        let addr = self.addr.clone();
        let fut = async move { Ok(Metadata { start_time, addr }) };
        Box::pin(fut)
    }
}
