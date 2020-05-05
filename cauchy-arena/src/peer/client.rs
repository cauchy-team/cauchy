use std::{pin::Pin, sync::Arc};

use futures::{
    channel::mpsc,
    prelude::*,
    task::{Context, Poll},
};
use network::codec::{Status, Transactions};
use network::Message;
use pin_project::pin_project;

use tower::Service;

use super::*;
use crate::FutResponse;

pub type ClientService =
    Buffer<Client<ClientTransport, TowerError<ClientTransport>, Message>, Message>;
#[derive(Clone)]
pub struct PeerClient {
    metadata: Arc<Metadata>,
    client_svc: ClientService,
    last_status: Arc<RwLock<Option<Status>>>,
}

impl PeerClient {
    pub fn new(
        metadata: Arc<Metadata>,
        last_status: Arc<RwLock<Option<Status>>>,
        client_svc: ClientService,
    ) -> Self {
        Self {
            metadata,
            client_svc,
            last_status,
        }
    }
}

impl PeerMetadata for PeerClient {
    fn get_metadata(&self) -> Arc<Metadata> {
        self.metadata.clone()
    }

    fn get_socket(&self) -> SocketAddr {
        self.metadata.addr.clone()
    }
}

#[pin_project]
pub struct ClientTransport {
    /// Incoming responses
    #[pin]
    sink: mpsc::Sender<Message>,
    /// Outgoing requests
    #[pin]
    stream: mpsc::Receiver<Message>,
}

impl ClientTransport {
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

#[derive(Debug)]
pub struct PollStatus;

pub enum PollStatusError {
    UnexpectedResponse,
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
        println!("Sending poll message to client");
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
        Box::pin(fut)
    }
}

pub struct Reconcile(Minisketch);

pub enum ReconcileError {
    UnexpectedResponse,
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

    fn call(&mut self, _: Reconcile) -> Self::Future {
        let response_fut = self.client_svc.call(Message::Poll);

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

#[derive(Debug)]
pub struct MetadataError;

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
