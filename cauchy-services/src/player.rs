use std::{pin::Pin, sync::Arc, time::Instant};

use futures::{
    channel::mpsc,
    prelude::*,
    task::{Context, Poll},
};
use network::codec::*;
use network::{FramedStream, Message};
use tokio::{net::TcpStream, sync::RwLock};
use tokio_tower::pipeline::Server;
use tokio_util::codec::Framed;
use tower::Service;

use crate::{
    arena::*,
    database::{Database, Error as DatabaseError},
    peers::*,
};

pub type TowerError = tokio_tower::Error<FramedStream, Message>;

pub type SplitStream = futures::stream::SplitStream<FramedStream>;

#[derive(Clone)]
pub struct Player {
    arena: Arena,
    metadata: Arc<Metadata>,
    database: Database,
    _state_svc: (),
    last_status: Arc<RwLock<Option<Status>>>,
}

impl Player {
    fn process_tcp_stream(&self, tcp_stream: TcpStream) -> Result<PeerHandle, std::io::Error> {
        let addr = tcp_stream.peer_addr()?;

        let codec = MessageCodec::default();
        let framed = Framed::new(tcp_stream, codec);

        let (response_sink, response_stream) = mpsc::channel(BUFFER_SIZE);
        let (request_sink, request_stream) = mpsc::channel(BUFFER_SIZE);

        let transport = server::Transport::new(framed, request_stream);
        let server = server::PeerServer::new(response_sink, self.clone());

        let metadata = Arc::new(Metadata {
            start_time: Instant::now(),
            addr,
        });
        let client = client::PeerClient::new(metadata, request_sink, response_stream);
        Ok(PeerHandle {
            server,
            client,
            transport,
        })
    }
}

impl Player {
    pub fn new(arena: Arena, metadata: Arc<Metadata>, database: Database) -> Self {
        Self {
            arena,
            metadata,
            database,
            _state_svc: (),
            last_status: Default::default(),
        }
    }
}

pub enum HandleError {
    Socket(std::io::Error),
    Arena,
}

const BUFFER_SIZE: usize = 128;

pub struct PeerHandle {
    server: server::PeerServer,
    client: client::PeerClient,
    transport: server::Transport,
}

impl Service<TcpStream> for Player {
    type Response = ();
    type Error = HandleError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Arena as Service<client::PeerClient>>::poll_ready(&mut self.arena, cx)
            .map_err(|()| HandleError::Arena)
    }

    fn call(&mut self, tcp_stream: TcpStream) -> Self::Future {
        let PeerHandle {
            server,
            client,
            transport,
        } = match self
            .process_tcp_stream(tcp_stream)
            .map_err(HandleError::Socket)
        {
            Ok(ok) => ok,
            Err(err) => return Box::pin(async move { Err(err) }),
        };

        // Spawn new peer server
        let server_fut = Server::new(transport, server);
        tokio::spawn(server_fut);

        Box::pin(async move { Ok(()) })
    }
}

impl Service<GetStatus> for Player {
    type Response = (Status, Marker);
    type Error = MissingStatus;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetStatus) -> Self::Future {
        let last_status_inner = self.last_status.clone();
        let fut = async move {
            last_status_inner
                .read()
                .await
                .clone()
                .ok_or(MissingStatus)
                .map(move |status| (status, Marker))
        };
        Box::pin(fut)
    }
}

pub enum TransactionError {
    Database(DatabaseError),
}

impl Service<TransactionInv> for Player {
    type Response = Transactions;
    type Error = TransactionError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO: Check for database locks
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, inv: TransactionInv) -> Self::Future {
        unimplemented!()
    }
}

impl Service<GetMetadata> for Player {
    type Response = Arc<Metadata>;
    type Error = ();
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetMetadata) -> Self::Future {
        let metadata = self.metadata.clone();
        let fut = async move { Ok(metadata) };
        Box::pin(fut)
    }
}
