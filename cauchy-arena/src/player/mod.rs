use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use bytes::Bytes;
use futures::{
    prelude::*,
    task::{Context, Poll},
};
use network::codec::*;
use network::FramedStream;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::RwLock,
};
use tokio_tower::pipeline::Server;
use tower::{util::ServiceExt, Service};

use super::*;
use crate::{arena::*, peer::*};
use database::{Database, Error as DatabaseError};
use miner::MiningCoordinator;

pub type SplitStream = futures::stream::SplitStream<FramedStream>;

const DIGEST_LEN: usize = 32;

#[derive(Clone)]
pub struct MiningReport {
    pub oddsketch: Bytes,
    pub root: Bytes,
    pub best_nonce: Arc<AtomicU64>,
}

impl Default for MiningReport {
    fn default() -> Self {
        Self {
            oddsketch: Bytes::new(),
            root: Bytes::from(vec![0; DIGEST_LEN]),
            best_nonce: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl MiningReport {
    fn to_status(&self) -> Status {
        Status {
            oddsketch: self.oddsketch.clone(),
            root: self.root.clone(),
            nonce: self.best_nonce.load(Ordering::SeqCst),
        }
    }
}

/// Player service
#[derive(Clone)]
pub struct Player {
    arena: Arena,
    metadata: Arc<Metadata>,
    mining_coordinator: MiningCoordinator,
    mining_report: Arc<RwLock<MiningReport>>,
    database: Database,
    _state_svc: (),
}

impl Player {
    pub async fn new(
        arena: Arena,
        mut mining_coordinator: MiningCoordinator,
        database: Database,
        metadata: Arc<Metadata>,
    ) -> Self {
        // TODO: Add pubkey
        let oddsketch = Bytes::new();
        let root = Bytes::from(vec![0; DIGEST_LEN]);
        let site = miner::RawSite::default();
        let best_nonce = mining_coordinator
            .call(miner::NewSession(site))
            .await
            .unwrap();

        let mining_report = MiningReport {
            oddsketch,
            root,
            best_nonce,
        };
        Self {
            arena,
            metadata,
            mining_coordinator,
            database,
            _state_svc: (),
            mining_report: Arc::new(RwLock::new(mining_report)),
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
        println!("getting status from player");
        let report_inner = self.mining_report.clone();

        let fut = async move {
            println!("getting status from player");
            let site = report_inner.read().await;
            let status = site.to_status();
            println!("status: {:?}", status);
            Ok((status, Marker))
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

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO: Check for database locks
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _inv: TransactionInv) -> Self::Future {
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

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, (_marker, _minisketch): (Marker, Minisketch)) -> Self::Future {
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
