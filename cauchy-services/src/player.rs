use std::sync::Arc;

use futures::{
    prelude::*,
    task::{Context, Poll},
};
use network::codec::*;
use network::{FramedStream, Message};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::RwLock,
};
use tokio_tower::pipeline::Server;
use tower::{util::ServiceExt, Service};

use super::*;
use crate::{
    arena::*,
    database::{Database, Error as DatabaseError},
    peer::*,
};

pub type TowerError = tokio_tower::Error<FramedStream, Message>;

pub type SplitStream = futures::stream::SplitStream<FramedStream>;

/// Player service
#[derive(Clone)]
pub struct Player {
    arena: Arena,
    metadata: Arc<Metadata>,
    database: Database,
    _state_svc: (),
    last_status: Arc<RwLock<Option<Status>>>,
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

    pub async fn begin_acceptor(self) {
        // Listen for peers
        let mut listener = TcpListener::bind(self.metadata.addr)
            .await
            .expect("failed to bind address");
        let filtered_listener = listener
            .incoming()
            .filter_map(|res| async move { res.ok() });
        // let mut boxed_listener = Box::pin(self.call_all(filtered_listener));
        let mut boxed_listener = Box::pin(filtered_listener);

        while let Some(tcp_stream) = boxed_listener.next().await {
            self.clone().oneshot(tcp_stream).await;
        }
    }
}

#[derive(Debug)]
pub enum HandleError {
    Socket(std::io::Error),
    Arena(arena::NewPeerError),
}

impl std::fmt::Display for HandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Socket(err) => err.fmt(f),
            Self::Arena(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for HandleError {}

impl Service<TcpStream> for Player {
    type Response = ();
    type Error = HandleError;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Arena as Service<Peer>>::poll_ready(&mut self.arena, cx).map_err(HandleError::Arena)
    }

    fn call(&mut self, tcp_stream: TcpStream) -> Self::Future {
        let (peer, transport) =
            match Peer::new(self.clone(), tcp_stream).map_err(HandleError::Socket) {
                Ok(ok) => ok,
                Err(err) => return Box::pin(async move { Err::<(), HandleError>(err) }),
            };

        // Add to peer list
        let mut arena = self.arena.clone();
        let fut = async move {
            if let Err(err) = arena.call(peer.clone()).map_err(HandleError::Arena).await {
                Err(err)
            } else {
                // Spawn new peer server
                let server_fut = Server::new(transport, peer);
                tokio::spawn(server_fut);

                Ok(())
            }
        };

        Box::pin(fut)
    }
}

impl Service<GetStatus> for Player {
    type Response = (Status, Marker);
    type Error = MissingStatus;
    type Future = FutResponse<Self::Response, Self::Error>;

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

#[derive(Debug)]
pub enum TransactionError {
    Database(DatabaseError),
}

impl Service<TransactionInv> for Player {
    type Response = Transactions;
    type Error = TransactionError;
    type Future = FutResponse<Self::Response, Self::Error>;

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

#[derive(Debug)]
pub enum ReconcileError {
    Database(DatabaseError),
}

impl Service<(Marker, Minisketch)> for Player {
    type Response = Transactions;
    type Error = ReconcileError;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, (marker, minisketch): (Marker, Minisketch)) -> Self::Future {
        todo!()
    }
}

/// Query arena
pub struct ArenaQuery<T>(pub T);

impl<T> Service<ArenaQuery<T>> for Player
where
    Arena: Service<T>,
    T: 'static + Send,
{
    type Response = <Arena as Service<T>>::Response;
    type Error = <Arena as Service<T>>::Error;
    type Future = <Arena as Service<T>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.arena.poll_ready(cx)
    }

    fn call(&mut self, ArenaQuery(req): ArenaQuery<T>) -> Self::Future {
        self.arena.call(req)
    }
}
