use std::pin::Pin;

use futures::{
    channel::mpsc,
    prelude::*,
    task::{Context, Poll},
};
use network::codec::Transactions;
use network::{codec::*, FramedStream, Message};
use pin_project::pin_project;
use tower::Service;

use super::*;
use crate::player;

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