use std::pin::Pin;

use futures::{
    channel::mpsc,
    prelude::*,
    task::{Context, Poll},
};
use network::{codec::*, FramedStream, Message};
use pin_project::pin_project;
use tower_service::Service;

use super::{
    player::{Player, TransactionError},
    GetStatus, MissingStatus,
};

pub type TowerError = tokio_tower::Error<FramedStream, Message>;
pub type SplitStream = futures::stream::SplitStream<FramedStream>;

#[pin_project]
pub struct Transport {
    #[pin]
    stream: SplitStream,
    #[pin]
    sink: mpsc::Sender<Option<Message>>,
}

impl Stream for Transport {
    type Item = Result<Message, DecodeError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().stream.poll_next(cx)
    }
}

impl Sink<Option<Message>> for Transport {
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

impl Transport {
    // Inject request stream into FramedStream
    pub fn new(framed: FramedStream, request_stream: mpsc::Receiver<Message>) -> Self {
        let (old_sink, stream) = framed.split();
        let (sink, new_stream) = mpsc::channel::<Option<Message>>(BUFFER_SIZE);

        // Forward request stream (from client) into sink
        let sink_inner = sink.clone();
        let fut_a = async move {
            let forward = request_stream
                .map(move |ok: Message| Ok(Some(ok)))
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

fn test(framed_stream: FramedStream) {
    let (response_sink, _) = mpsc::channel(BUFFER_SIZE);
    let (_, request_stream) = mpsc::channel(BUFFER_SIZE);

    let socket_addr: std::net::SocketAddr = "0.0.0.0:123".parse().unwrap();
    let database = Default::default();
    let player = Player::new(socket_addr, database);
    let peer = PeerServer::new(response_sink, player);
    let transport = Transport::new(framed_stream, request_stream);
    let server = tokio_tower::pipeline::Server::new(transport, peer);
    tokio::spawn(server);
}

#[derive(Clone)]
pub struct PeerServer {
    _state_svc: (),
    player: Player,
    response_sink: mpsc::Sender<Message>,
}

impl PeerServer {
    pub fn new(response_sink: mpsc::Sender<Message>, player: Player) -> Self {
        Self {
            _state_svc: (),
            player,
            response_sink,
        }
    }
}

pub enum Error {
    ResponseSend(mpsc::SendError),
    MissingStatus(MissingStatus),
    Transaction(TransactionError),
}

impl Service<Message> for PeerServer {
    type Response = Option<Message>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, message: Message) -> Self::Future {
        let mut this = self.clone();
        let fut = async move {
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
                    let status = this.player.call(GetStatus).await;
                    status
                        .map(|ok| Some(Message::Status(ok)))
                        .map_err(Error::MissingStatus)
                }
                Message::TransactionInv(inv) => {
                    let transactions: Result<Transactions, _> = this.player.call(inv).await;
                    transactions
                        .map(|ok| Some(Message::Transactions(ok)))
                        .map_err(Error::Transaction)
                }
                Message::Reconcile => todo!(),
            }
        };
        Box::pin(fut)
    }
}
