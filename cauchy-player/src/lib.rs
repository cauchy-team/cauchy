pub mod peer;

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};

use bytes::Bytes;
use futures_channel::mpsc;
use futures_core::task::{Context, Poll};
use futures_util::stream::StreamExt;
use network::{codec::*, FramedStream};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::RwLock,
};
use tokio_tower::pipeline::{Client, Server};
use tokio_util::codec::Framed;
use tower_buffer::Buffer;
use tower_service::Service;
use tower_util::ServiceExt;
use tracing::info;

use common::*;
use consensus::Entry;
use database::{Database, Error as DatabaseError};
use miner::MiningCoordinator;
use peer::{Peer, PeerClient};

pub type SplitStream = futures_util::stream::SplitStream<FramedStream>;

const DIGEST_LEN: usize = 32;

#[derive(Clone)]
pub struct StateSnapshot {
    pub oddsketch: Bytes,
    pub root: Bytes,
    pub minisketch: Minisketch,
    pub best_nonce: Arc<AtomicU64>,
}

impl StateSnapshot {
    fn to_parts(&self) -> (Minisketch, Status) {
        let status = Status {
            oddsketch: self.oddsketch.clone(),
            root: self.root.clone(),
            nonce: self.best_nonce.load(Ordering::SeqCst) as u64, // TODO: Fix
        };

        let minisketch = self.minisketch.clone();
        (minisketch, status)
    }
}

/// Player service
#[derive(Clone)]
pub struct Player<A> {
    arena: A,
    metadata: Arc<Metadata>,
    mining_coordinator: MiningCoordinator,
    state_snapshot: Arc<RwLock<StateSnapshot>>,
    database: Database,
    _state_svc: (),
}

pub trait PeerConstructor {
    fn new(&self, tcp_stream: TcpStream) -> Result<PeerClient, std::io::Error>;
}

const PEER_BUFFER: usize = 128;

impl<A> Service<TcpStream> for Player<A>
where
    A: Clone + Send + Sync + 'static,
    A: Service<(SocketAddr, PeerClient)>,
    <A as Service<(SocketAddr, PeerClient)>>::Future: Send,
{
    type Response = ();
    type Error = std::io::Error;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
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

        let (response_sink, response_stream) = mpsc::channel(PEER_BUFFER);
        let (request_sink, request_stream) = mpsc::channel(PEER_BUFFER);

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
    // Arena peer constructor interface
    A: Service<(SocketAddr, PeerClient)>,
    <A as Service<(SocketAddr, PeerClient)>>::Future: Send,
    // Poll interface
    A: Service<SampleQuery<PollStatus>>,
    <A as Service<SampleQuery<PollStatus>>>::Response:
        std::fmt::Debug + IntoIterator<Item = (SocketAddr, Status)>,
    <A as Service<SampleQuery<PollStatus>>>::Error: std::fmt::Debug,
    A: Service<DirectedQuery<Reconcile>>,
{
    pub async fn new(
        bind_addr: SocketAddr,
        arena: A,
        mut mining_coordinator: MiningCoordinator,
        database: Database,
        radius: usize,
    ) -> Self {
        // Collect metadata
        let start_time = std::time::SystemTime::now();
        let metadata = Arc::new(Metadata {
            addr: bind_addr.clone(),
            start_time,
        });

        // TODO: Add pubkey
        let oddsketch = Bytes::from(vec![1; 4 * radius]);
        let minisketch = Minisketch(Bytes::new());
        let root = Bytes::from(vec![0; DIGEST_LEN]);
        let site = miner::RawSite::default();
        let best_nonce = mining_coordinator
            .call(miner::NewSession(site))
            .await
            .unwrap();

        let state_snapshot = StateSnapshot {
            minisketch,
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
            state_snapshot: Arc::new(RwLock::new(state_snapshot)),
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

    pub async fn begin_heartbeat(self, sample_size: usize, interval_ms: u64) {
        // Poll sample
        let mut timer = tokio::time::interval(Duration::from_millis(interval_ms));
        let query = SampleQuery(PollStatus, sample_size);
        while let Some(_) = timer.next().await {
            info!("starting heartbeat");
            // Aggregate results
            let peer_statuses = self.arena.clone().oneshot(query.clone()).await.unwrap(); // TODO: Don't unwrap
            let (_marker, player_status) = self.clone().oneshot(GetStatus).await.unwrap(); // TODO: Don't unwrap
            let (addrs, mut peer_entries): (Vec<_>, Vec<_>) = peer_statuses
                .into_iter()
                .map(move |(addr, status)| (addr, Entry::from_site(&[], status)))
                .unzip();

            let my_pubkey = &[];
            peer_entries.push(Entry::from_site(my_pubkey, player_status));

            let winning_index = consensus::calculate_winner(&peer_entries[..]).unwrap(); // TODO: Don't unwrap
            info!("{} of {} wins", winning_index + 1, peer_entries.len());
            if peer_entries.len() == winning_index + 1 {
                info!("player won with {:?}", peer_entries[winning_index]);
            } else {
                let addr = addrs[winning_index];
                info!(
                    "{:?} won with {:?}",
                    addrs[winning_index], peer_entries[winning_index]
                );
                let (minisketch, _) = self.state_snapshot.read().await.to_parts();
                let reconcile_query = DirectedQuery(addr, Reconcile(minisketch));
                self.arena.clone().oneshot(reconcile_query).await;
            }
        }
    }
}

impl<A> Service<GetStatus> for Player<A> {
    type Response = (Minisketch, Status);
    type Error = MissingStatus;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetStatus) -> Self::Future {
        let report_inner = self.state_snapshot.clone();
        let fut = async move {
            let site = report_inner.read().await;
            let (minisketch, status) = site.to_parts();
            info!("fetched status from player; {:?}", status);
            Ok((minisketch, status))
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

impl<A> Service<Minisketch> for Player<A> {
    type Response = Transactions;
    type Error = ReconcileError;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _minisketch: Minisketch) -> Self::Future {
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

pub struct MempoolError;

impl<A> Service<Transaction> for Player<A> {
    type Response = ();
    type Error = MempoolError;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Transaction) -> Self::Future {
        // TODO: Send to VM service
        // TODO: Get new root


        let state_snapshot = self.state_snapshot.clone();
        let fut = async move {
            let StateSnapshot {
                oddsketch,
                minisketch,
                best_nonce,
                root
            } = &*state_snapshot.write().await;

        };
        // Box::pin(fut)
        todo!()
    }
}