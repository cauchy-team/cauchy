use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

use crypto::blake3;
use futures_core::task::{Context, Poll};
use parking_lot::Mutex;
use tokio::sync::RwLock;
use tower_service::Service;

pub type FutResponse<T, E> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>;

pub type Site = [u8; blake3::OUT_LEN];
pub type Digest = [u8; 32];

pub const WORST_DIGEST: [u8; 32] = [0; 32];

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[derive(Clone)]
pub struct Miner {
    pub site: Site,
    pub best_nonce: Arc<AtomicU64>,
    pub best_digest: Arc<Mutex<Digest>>,
    pub pool: Arc<rayon::ThreadPool>,
    pub terminator: Arc<AtomicBool>,
}

impl Miner {
    /// When `n_threads` is zero then it defaults to the number of CPUs.
    pub fn new(
        site: Site,
        best_nonce: Arc<AtomicU64>,
        best_digest: Arc<Mutex<Digest>>,
        pool: Arc<rayon::ThreadPool>,
        terminator: Arc<AtomicBool>,
    ) -> Self {
        Self {
            site,
            best_nonce,
            best_digest,
            pool,
            terminator,
        }
    }

    /// Begin mining at site.
    pub fn spawn(&self, offset: u64) {
        // Prime hasher with site preimage
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.site);

        let best_nonce_inner = self.best_nonce.clone();
        let best_digest = self.best_digest.clone();
        let terminator_inner = self.terminator.clone();
        self.pool.spawn(move || {
            let mut best = WORST_DIGEST;
            for nonce in offset.. {
                if false == terminator_inner.load(Ordering::Relaxed) {
                    let digest = *hasher
                        .clone()
                        .update(&nonce.to_be_bytes())
                        .finalize()
                        .as_bytes();

                    if best < digest {
                        best = digest;
                        let mut best_digest_locked = best_digest.lock();
                        if *best_digest_locked < digest {
                            // Store record
                            best_nonce_inner.store(nonce, Ordering::SeqCst);
                            *best_digest_locked = digest;
                        }
                    }
                } else {
                    break;
                }
            }
        });
    }
}

pub struct GetNonce;

impl Service<GetNonce> for Miner {
    type Response = u64;
    type Error = ();
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetNonce) -> Self::Future {
        let best_nonce = self.best_nonce.load(Ordering::SeqCst);
        Box::pin(async move { Ok(best_nonce) })
    }
}

#[derive(Clone)]
pub struct MiningCoordinator {
    pool: Arc<rayon::ThreadPool>,
    terminators: Arc<Mutex<Vec<Arc<AtomicBool>>>>,
    n_workers: u32,
    current_miner: Arc<RwLock<Option<Miner>>>,
}

impl MiningCoordinator {
    pub fn new(n_workers: u32) -> Self {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(n_workers as usize)
            .build()
            .unwrap();
        Self {
            pool: Arc::new(pool),
            terminators: Default::default(),
            n_workers,
            current_miner: Default::default(),
        }
    }

    pub async fn current_miner(&self) -> Option<Miner> {
        (*self.current_miner.read().await).clone()
    }

    pub fn n_workers(&self) -> u32 {
        self.n_workers
    }
}

pub struct NewSession(pub Site);

impl Service<NewSession> for MiningCoordinator {
    type Response = Arc<AtomicU64>;
    type Error = ();
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, NewSession(site): NewSession) -> Self::Future {
        // Stop workers
        let mut terminators = self.terminators.lock();
        while let Some(atomic) = terminators.pop() {
            atomic.store(true, Ordering::Relaxed);
        }

        // Create miner
        let best_nonce = Arc::new(AtomicU64::new(0));
        let best_digest = Arc::new(Mutex::new(WORST_DIGEST));
        let terminator = Arc::new(AtomicBool::new(false));
        let miner = Miner::new(
            site,
            best_nonce.clone(),
            best_digest,
            self.pool.clone(),
            terminator,
        );

        // Spawn workers
        let n_workers = self.n_workers as u64;
        for i in 0..n_workers {
            let offset = (std::u64::MAX / n_workers) * i;
            miner.spawn(offset);
        }

        // Switch miners
        let current_miner = self.current_miner.clone();
        Box::pin(async move {
            *current_miner.write().await = Some(miner);
            Ok(best_nonce)
        })
    }
}
