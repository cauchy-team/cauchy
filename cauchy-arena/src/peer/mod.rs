use std::{net::SocketAddr, pin::Pin, sync::Arc, time::SystemTime};

use futures::{
    channel::mpsc,
    prelude::*,
    task::{Context, Poll},
};
use network::codec::{Status, Transactions};
use network::{codec::*, FramedStream, Message};
use pin_project::pin_project;
use tokio::{
    net::TcpStream,
    sync::{Mutex, RwLock},
};
use tokio_tower::pipeline::Client;
use tokio_util::codec::Framed;
use tower::buffer::Buffer;
use tower::Service;

use super::*;
use crate::player;

pub type TowerError<T> = tokio_tower::Error<T, Message>;

pub type SplitStream = futures::stream::SplitStream<FramedStream>;

#[pin_project]
pub struct ServerTransport {
    /// Incoming messages
    #[pin]
    stream: SplitStream,
    /// Outgoing messages
    #[pin]
    sink: mpsc::Sender<Option<Message>>,
}

impl Stream for ServerTransport {
    type Item = Result<Message, DecodeError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().stream.poll_next(cx)
    }
}

impl Sink<Option<Message>> for ServerTransport {
    type Error = mpsc::SendError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sink.poll_ready(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sink.poll_close(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Option<Message>) -> Result<(), Self::Error> {
        self.project().sink.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sink.poll_flush(cx)
    }
}

const BUFFER_SIZE: usize = 128;

impl ServerTransport {
    // Inject request stream into FramedStream
    pub fn new(framed: FramedStream, request_stream: mpsc::Receiver<Message>) -> Self {
        let (old_sink, stream) = framed.split();
        let (sink, new_stream) = mpsc::channel::<Option<Message>>(BUFFER_SIZE);

        // Forward request stream (from client) into sink
        let sink_inner = sink.clone();
        let fut_a = async move {
            let forward = request_stream
                .map(move |ok: Message| {
                    println!("forwarding message {:?}", ok);
                    Ok(Some(ok))
                })
                .forward(sink_inner);
            forward.await
        };

        // Forward new_stream into old_sink
        let fut_b = async move {
            let forward = new_stream
                .filter_map(move |opt| async move { opt.map(|some| Ok(some)) })
                .forward(old_sink);
            forward.await
        };
        tokio::spawn(fut_a);
        tokio::spawn(fut_b);
        Self { stream, sink }
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

#[derive(Clone)]
pub struct Peer {
    metadata: Arc<Metadata>,
    player: player::Player,
    perception: Arc<Mutex<Option<Marker>>>,
    response_sink: mpsc::Sender<Message>,
    last_status: Arc<RwLock<Option<Status>>>,
    client_svc: Buffer<Client<ClientTransport, TowerError<ClientTransport>, Message>, Message>,
}

impl Peer {
    pub fn new(
        player: player::Player,
        tcp_stream: TcpStream,
    ) -> Result<(Self, ServerTransport), std::io::Error> {
        let addr = tcp_stream.peer_addr()?;

        let codec = MessageCodec::default();
        let framed = Framed::new(tcp_stream, codec);

        let (response_sink, response_stream) = mpsc::channel(BUFFER_SIZE);
        let (request_sink, request_stream) = mpsc::channel(BUFFER_SIZE);

        let server_transport = ServerTransport::new(framed, request_stream);

        let client_transport = ClientTransport::new(request_sink, response_stream);
        let client_svc = Buffer::new(Client::new(client_transport), BUFFER_SIZE);

        let metadata = Arc::new(Metadata {
            start_time: SystemTime::now(),
            addr,
        });
        Ok((
            Self {
                metadata,
                player,
                perception: Default::default(),
                response_sink,
                last_status: Default::default(),
                client_svc,
            },
            server_transport,
        ))
    }
}

pub enum Error {
    ResponseSend(mpsc::SendError),
    MissingStatus(MissingStatus),
    Reconcile(player::ReconcileError),
    Transaction(player::TransactionError),
    GetStatus(MissingStatus),
    TransactionInv(player::TransactionError),
    UnexpectedReconcile,
}

impl Service<Message> for Peer {
    type Response = Option<Message>;
    type Error = Error;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match <player::Player as Service<GetStatus>>::poll_ready(&mut self.player, cx) {
            Poll::Ready(Ok(_)) => (),
            Poll::Ready(Err(err)) => return Poll::Ready(Err(Error::GetStatus(err))),
            Poll::Pending => return Poll::Pending,
        }

        match <player::Player as Service<TransactionInv>>::poll_ready(&mut self.player, cx) {
            Poll::Ready(Ok(_)) => (),
            Poll::Ready(Err(err)) => return Poll::Ready(Err(Error::TransactionInv(err))),
            Poll::Pending => return Poll::Pending,
        }

        match <player::Player as Service<(Marker, Minisketch)>>::poll_ready(&mut self.player, cx) {
            Poll::Ready(Ok(_)) => (),
            Poll::Ready(Err(err)) => return Poll::Ready(Err(Error::Reconcile(err))),
            Poll::Pending => return Poll::Pending,
        }

        Poll::Ready(Ok(()))
    }

    fn call(&mut self, message: Message) -> Self::Future {
        let mut this = self.clone();
        let fut = async move {
            println!("got message {:?}", message);
            match message {
                // Send responses
                Message::ReconcileResponse(_)
                | Message::Status(_)
                | Message::Transactions(_)
                | Message::Transaction(_) => this
                    .response_sink
                    .send(message)
                    .await
                    .map_err(Error::ResponseSend)
                    .map(|_| None),
                Message::Poll => {
                    let (status, marker) = match this.player.call(GetStatus).await {
                        Ok(ok) => ok,
                        Err(err) => return Err(Error::MissingStatus(err)),
                    };
                    println!("fetched status {:?}", status);
                    *this.perception.clone().lock().await = Some(marker);
                    println!("responding...");
                    return Ok(Some(Message::Status(status)));
                }
                Message::TransactionInv(inv) => {
                    let transactions: Result<Transactions, _> = this.player.call(inv).await;
                    transactions
                        .map(|ok| Some(Message::Transactions(ok)))
                        .map_err(Error::Transaction)
                }
                Message::Reconcile => {
                    let marker = this
                        .perception
                        .lock()
                        .await
                        .take()
                        .ok_or(Error::UnexpectedReconcile)?;
                    let transactions: Result<Transactions, _> =
                        this.player.call((marker, Minisketch)).await;
                    transactions
                        .map(|ok| Some(Message::Transactions(ok)))
                        .map_err(Error::Reconcile)
                }
            }
        };
        Box::pin(fut)
    }
}

impl Peer {
    pub fn get_metadata(&self) -> Arc<Metadata> {
        self.metadata.clone()
    }

    pub fn get_socket(&self) -> SocketAddr {
        self.metadata.addr.clone()
    }
}

pub struct PollStatus;

pub enum PollStatusError {
    UnexpectedResponse,
    Tower(Box<dyn std::error::Error + Send + Sync>),
}

impl Service<PollStatus> for Peer {
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

impl Service<Reconcile> for Peer {
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

impl Service<GetStatus> for Peer {
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

impl Service<GetMetadata> for Peer {
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
