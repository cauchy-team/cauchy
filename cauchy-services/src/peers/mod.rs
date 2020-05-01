pub mod client;
pub mod player;
pub mod server;

use std::{net::SocketAddr, pin::Pin, sync::Arc, time::Instant};

use futures::channel::mpsc;
use futures::{
    prelude::*,
    task::{Context, Poll},
};
use network::codec::*;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tower::Service;

pub struct Marker;
pub struct Minisketch;

pub struct GetStatus;

pub struct MissingStatus;

pub struct GetMetadata;

pub struct Metadata {
    pub start_time: Instant,
    pub addr: SocketAddr,
}

pub struct PeerFactory {
    player: player::Player,
}

pub enum HandleError {
    Socket(std::io::Error),
}

const BUFFER_SIZE: usize = 128;

pub struct PeerHandle {
    server: server::PeerServer,
    client: client::PeerClient,
    transport: server::Transport,
}

impl Service<TcpStream> for PeerFactory {
    type Response = PeerHandle;
    type Error = HandleError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TcpStream) -> Self::Future {
        let addr = match req.peer_addr() {
            Ok(ok) => ok,
            Err(err) => return Box::pin(async move { Err(HandleError::Socket(err)) }),
        };

        let codec = MessageCodec::default();
        let framed = Framed::new(req, codec);

        let (response_sink, response_stream) = mpsc::channel(BUFFER_SIZE);
        let (request_sink, request_stream) = mpsc::channel(BUFFER_SIZE);

        let transport = server::Transport::new(framed, request_stream);
        let server = server::PeerServer::new(response_sink, self.player.clone());

        let metadata = Arc::new(Metadata {
            start_time: Instant::now(),
            addr,
        });
        let client = client::PeerClient::new(metadata, request_sink, response_stream);
        let fut = async move {
            Ok(PeerHandle {
                server,
                client,
                transport,
            })
        };
        Box::pin(fut)
    }
}
