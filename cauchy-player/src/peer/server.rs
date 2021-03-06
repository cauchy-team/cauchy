use std::pin::Pin;

use futures_channel::mpsc;
use futures_core::{
    stream::Stream,
    task::{Context, Poll},
};
use futures_sink::Sink;
use futures_util::{sink::SinkExt, stream::StreamExt};
use pin_project::pin_project;
use tokio::sync::Mutex;
use tower_service::Service;
use tracing::info;

use crate::*;
use common::{network::*, services::*};
use network::{FramedStream, Message};

type SplitStream = futures_util::stream::SplitStream<FramedStream>;

/// Underlying transport for the `PeerClient`. Used to forward messages to a remote peer.
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

pub const BUFFER_SIZE: usize = 128;

impl ServerTransport {
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

#[derive(Clone)]
pub struct PeerServer<Pl> {
    pub player: Pl,
    pub perception: Arc<Mutex<Option<Minisketch>>>,
    pub response_sink: mpsc::Sender<Message>,
    pub radius: usize,
}

pub trait PeerMetadata {
    fn get_metadata(&self) -> Arc<Metadata>;
    fn get_socket(&self) -> SocketAddr;
}

pub enum Error {
    ResponseSend(mpsc::SendError),
    MissingStatus(MissingStatus),
    Transaction(TransactionError),
    GetStatus(MissingStatus),
    TransactionInv(TransactionError),
    Minisketch(MinisketchError),
    UnexpectedReconcile,
}

impl<Pl> Service<Message> for PeerServer<Pl>
where
    Pl: Clone + Send + 'static,
    // Get status from player
    Pl: Service<GetStatus, Response = (Minisketch, Status), Error = MissingStatus>,
    <Pl as Service<GetStatus>>::Future: Send,
    // Get transaction from player
    Pl: Service<TransactionInv, Response = Transactions, Error = TransactionError>,
    <Pl as Service<TransactionInv>>::Future: Send,
{
    type Response = Option<Message>;
    type Error = Error;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match <Pl as Service<GetStatus>>::poll_ready(&mut self.player, cx) {
            Poll::Ready(Ok(_)) => (),
            Poll::Ready(Err(err)) => return Poll::Ready(Err(Error::GetStatus(err))),
            Poll::Pending => return Poll::Pending,
        }

        match <Pl as Service<TransactionInv>>::poll_ready(&mut self.player, cx) {
            Poll::Ready(Ok(_)) => (),
            Poll::Ready(Err(err)) => return Poll::Ready(Err(Error::TransactionInv(err))),
            Poll::Pending => return Poll::Pending,
        }

        Poll::Ready(Ok(()))
    }

    fn call(&mut self, message: Message) -> Self::Future {
        let mut this = self.clone();
        let fut = async move {
            trace!("received message; {:?}", message);
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
                    let (minisketch, status) = match this.player.call(GetStatus).await {
                        Ok(ok) => ok,
                        Err(err) => return Err(Error::MissingStatus(err)),
                    };
                    *this.perception.clone().lock().await = Some(minisketch);

                    trace!("fetched status; {:?}", status);
                    return Ok(Some(Message::Status(status)));
                }
                Message::TransactionInv(inv) => {
                    let transactions: Result<Transactions, _> = this.player.call(inv).await;
                    transactions
                        .map(|ok| Some(Message::Transactions(ok)))
                        .map_err(Error::Transaction)
                }
                Message::Reconcile(minisketch) => {
                    let mut perceived_minisketch = this
                        .perception
                        .lock()
                        .await
                        .take()
                        .ok_or(Error::UnexpectedReconcile)?
                        .hydrate(this.radius)
                        .map_err(Error::Minisketch)?;
                    let peer_minisketch =
                        minisketch.hydrate(this.radius).map_err(Error::Minisketch)?;
                    perceived_minisketch
                        .merge(&peer_minisketch)
                        .map_err(Error::Minisketch)?;

                    let mut elements = vec![0; this.radius];
                    let n_ele = perceived_minisketch
                        .decode(&mut elements)
                        .map_err(Error::Minisketch)?;
                    info!("sending {} transactions", n_ele);

                    let txs = Transactions { txs: Vec::new() };

                    Ok(Some(Message::Transactions(txs)))
                }
            }
        };
        Box::pin(fut)
    }
}
