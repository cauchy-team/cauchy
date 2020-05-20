pub mod peer;

use std::{
    convert::TryInto,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};

use bytes::Bytes;
use crypto::blake3;
use dashmap::DashMap;
use futures_channel::mpsc;
use futures_core::task::{Context, Poll};
use futures_util::{future::abortable, stream::StreamExt};
use network::{codec::*, FramedStream};
use tokio::{net::TcpListener, sync::RwLock};
use tokio_tower::pipeline::{Client, Server};
use tokio_util::codec::Framed;
use tower_buffer::Buffer;
use tower_service::Service;
use tower_util::ServiceExt;
use tracing::{info, trace};

use common::{network::*, services::*, FutResponse};
use consensus::Entry;
use crypto::{Minisketch as MinisketchCrypto, MinisketchError, Oddsketch};
use database::{Database, Error as DatabaseError};
use miner::MiningCoordinator;
use peer::{Peer, PeerClient};

pub type SplitStream = futures_util::stream::SplitStream<FramedStream>;

const DIGEST_LEN: usize = 32;

#[derive(Clone)]
pub struct StateSnapshot {
    pub oddsketch: Bytes,
    pub root: Bytes,
    pub minisketch: Bytes,
    pub best_nonce: Arc<AtomicU64>,
}

impl StateSnapshot {
    fn to_parts(&self) -> (Minisketch, Status) {
        let status = Status {
            oddsketch: Bytes::from(self.oddsketch.clone()),
            root: self.root.clone(),
            nonce: self.best_nonce.load(Ordering::SeqCst) as u64, // TODO: Fix
        };

        let minisketch = self.minisketch.clone();
        (Minisketch(minisketch), status)
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
    txs: Arc<DashMap<[u8; blake3::OUT_LEN], Transaction>>,
    radius: usize,
}

const PEER_BUFFER: usize = 128;

/// Add new peer.
impl<A> Service<NewPeer> for Player<A>
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

    fn call(&mut self, NewPeer(tcp_stream): NewPeer) -> Self::Future {
        let addr = match tcp_stream.peer_addr() {
            Ok(ok) => ok,
            Err(err) => return Box::pin(async move { Err(err) }),
        };

        let codec = MessageCodec::default();
        let framed = Framed::new(tcp_stream, codec);

        let (response_sink, response_stream) = mpsc::channel(PEER_BUFFER);
        let (request_sink, request_stream) = mpsc::channel(PEER_BUFFER);

        // Server transport
        let server_transport = peer::ServerTransport::new(framed, request_stream);

        // Peer service
        let service = Peer {
            player: self.clone(),
            perception: Default::default(),
            response_sink,
            radius: self.radius,
        };

        let server = Server::new(server_transport, service);
        let (server_abortable, terminator) = abortable(server);
        tokio::spawn(server_abortable);

        let metadata = Arc::new(Metadata {
            start_time: SystemTime::now(),
            addr,
        });
        let client_transport = peer::ClientTransport::new(request_sink, response_stream);
        let client_svc = Buffer::new(Client::new(client_transport), peer::BUFFER_SIZE);
        let client = PeerClient::new(metadata, Default::default(), client_svc, terminator);

        let arena = self.arena.clone();
        let fut = async move {
            arena.oneshot((addr, client)).await;
            Ok(())
        };

        Box::pin(fut)
    }
}

/// Remove peer.
impl<A> Service<RemovePeer> for Player<A>
where
    A: Service<RemovePeer, Response = ()>,
    <A as Service<RemovePeer>>::Future: Send + 'static,
{
    type Response = ();
    type Error = <A as Service<RemovePeer>>::Error;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.arena.poll_ready(cx)
    }

    fn call(&mut self, request: RemovePeer) -> Self::Future {
        Box::pin(self.arena.call(request))
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
        let oddsketch = Bytes::from(vec![0; 4 * radius as usize]);
        let minisketch = Bytes::from(vec![0; 8 * radius]);
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
            txs: Default::default(),
            state_snapshot: Arc::new(RwLock::new(state_snapshot)),
            radius,
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
            this.clone().oneshot(NewPeer(tcp_stream)).await;
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
                .map(move |(addr, status)| (addr, Entry::from_status(&[], status)))
                .unzip();

            let my_pubkey = &[];
            peer_entries.push(Entry::from_status(my_pubkey, player_status));

            let winning_index = consensus::calculate_winner(&peer_entries[..]).unwrap(); // TODO: Don't unwrap
            if peer_entries.len() == winning_index + 1 {
                trace!("player won with {:?}", peer_entries[winning_index]);
            } else {
                let addr = addrs[winning_index];
                trace!(
                    "{:?} won with {:?}",
                    addrs[winning_index],
                    peer_entries[winning_index]
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
            trace!("fetched status from player; {:?}", status);
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

    fn call(&mut self, inv: TransactionInv) -> Self::Future {
        let tx_ids = inv.tx_ids;
        let txs: Vec<_> = tx_ids
            .iter()
            .filter_map(|tx_id| {
                let tx_id_arr: [u8; 32] = tx_id[..].try_into().unwrap();
                self.txs
                    .get(&tx_id_arr)
                    .map(move |pair| pair.value().clone())
            })
            .collect();
        let transactions = Transactions { txs };

        Box::pin(async move { Ok(transactions) })
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

#[derive(Debug)]
pub struct MempoolError;

impl<A> Service<Transaction> for Player<A> {
    type Response = ();
    type Error = MempoolError;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, tx: Transaction) -> Self::Future {
        info!("broadcasting transaction; {:?}", tx);
        // TODO: Send to VM service
        // TODO: Get new root

        let state_snapshot = self.state_snapshot.clone();
        let radius = self.radius;
        let fut = async move {
            let StateSnapshot {
                oddsketch,
                minisketch,
                best_nonce,
                root,
            } = &mut *state_snapshot.write().await;

            // Deserialize oddsketch
            let mut ms = MinisketchCrypto::try_new(64, 0, radius).unwrap(); // This is safe
            ms.deserialize(&minisketch);

            // Calculate short ID
            let short_id = tx.get_short_id();

            // Add to minisketch
            ms.add(short_id);

            let mut minisketch_raw = vec![0; ms.serialized_size()];
            ms.serialize(&mut minisketch_raw).unwrap(); // This is safe
            info!("new oddsketch: {:?}", minisketch_raw);
            *minisketch = Bytes::from(minisketch_raw);

            // Add to oddsketch
            let oddsketch_raw = oddsketch.to_vec();
            let mut new_oddsketch = Oddsketch::new(oddsketch_raw);
            new_oddsketch.insert(short_id);
            *oddsketch = Bytes::from(new_oddsketch.to_vec());
            info!("new oddsketch; {:?}", oddsketch);

            Ok(())
        };
        Box::pin(fut)
    }
}
