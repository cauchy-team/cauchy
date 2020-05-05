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
use tower::{util::ServiceExt, Service};

use super::*;
use database::{Database, Error as DatabaseError};
use miner::MiningCoordinator;
use std::{net::SocketAddr, time::SystemTime};

use futures::channel::mpsc;
use network::codec::Status;
use tokio_tower::pipeline::{Client, Server};
use tokio_util::codec::Framed;
use tower::buffer::Buffer;

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
pub struct Player<A> {
    arena: A,
    metadata: Arc<Metadata>,
    mining_coordinator: MiningCoordinator,
    mining_report: Arc<RwLock<MiningReport>>,
    database: Database,
    _state_svc: (),
}

pub trait PeerConstructor {
    fn new(&self, tcp_stream: TcpStream) -> Result<PeerClient, std::io::Error>;
}

impl<A> Service<TcpStream> for Player<A>
where
    A: Clone + Send + Sync + 'static,
    A: Service<(SocketAddr, PeerClient)>,
    <A as Service<(SocketAddr, PeerClient)>>::Future: Send,
{
    type Response = ();
    type Error = std::io::Error;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, tcp_stream: TcpStream) -> Self::Future {
        let addr = match tcp_stream.peer_addr() {
            Ok(ok) => ok,
            Err(err) => return Box::pin(async move { Err(err) }),
        };

        let codec = MessageCodec::default();
        let framed = Framed::new(tcp_stream, codec);

        let (response_sink, response_stream) = mpsc::channel(peer::BUFFER_SIZE);
        let (request_sink, request_stream) = mpsc::channel(peer::BUFFER_SIZE);

        let server_transport = peer::ServerTransport::new(framed, request_stream);
        let service = Peer {
            player: self.clone(),
            perception: Default::default(),
            response_sink,
        };

        let server = Server::new(server_transport, service);
        tokio::spawn(server);

        let metadata = Arc::new(Metadata {
            start_time: SystemTime::now(),
            addr,
        });
        let client_transport = peer::ClientTransport::new(request_sink, response_stream);
        let client_svc = Buffer::new(Client::new(client_transport), peer::BUFFER_SIZE);
        let client = PeerClient::new(metadata, Default::default(), client_svc);

        let arena = self.arena.clone();
        let fut = async move {
            arena.oneshot((addr, client)).await;
            Ok(())
        };

        Box::pin(fut)
    }
}

impl<A> Player<A>
where
    A: Clone + Default + Send + Sync + 'static,
    A: Service<(SocketAddr, PeerClient)>,
    <A as Service<(SocketAddr, PeerClient)>>::Future: Send,
{
    pub async fn new(
        bind_addr: SocketAddr,
        arena: A,
        mut mining_coordinator: MiningCoordinator,
        database: Database,
    ) -> Self {
        // Collect metadata
        let start_time = std::time::SystemTime::now();
        let metadata = Arc::new(Metadata {
            addr: bind_addr.clone(),
            start_time,
        });

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

        let this = self.clone();
        while let Some(tcp_stream) = boxed_listener.next().await {
            this.clone().oneshot(tcp_stream).await;
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

impl<A> Service<GetStatus> for Player<A> {
    type Response = (Marker, Status);
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
            Ok((Marker, status))
        };
        Box::pin(fut)
    }
}

#[derive(Debug)]
pub enum TransactionError {
    Database(DatabaseError),
}

impl<A> Service<TransactionInv> for Player<A> {
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

impl<A> Service<GetMetadata> for Player<A> {
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

impl<A> Service<(Marker, Minisketch)> for Player<A> {
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

impl<A, T> Service<ArenaQuery<T>> for Player<A>
where
    A: Service<T>,
    T: 'static + Send + Sized,
{
    type Response = <A as Service<T>>::Response;
    type Error = <A as Service<T>>::Error;
    type Future = <A as Service<T>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.arena.poll_ready(cx)
    }

    fn call(&mut self, ArenaQuery(req): ArenaQuery<T>) -> Self::Future {
        self.arena.call(req)
    }
}
