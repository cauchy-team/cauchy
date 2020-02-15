use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crypto::blake3;
use futures_channel::mpsc;
use parking_lot::RwLock;
use rayon::prelude::*;

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub const MINING_RECORD_BUFFER: usize = 512;
pub const MINING_CHUNK_SIZE: usize = 2048;

pub struct Mining {
    site: [u8; blake3::OUT_LEN],
    pool: rayon::ThreadPool,
}

#[derive(Debug)]
pub struct MiningRecord {
    pub nonce: u64,
    pub digest: [u8; blake3::OUT_LEN],
}

impl Mining {
    /// When `n_threads` is zero then it defaults to the number of CPUs.
    pub fn new(site: [u8; blake3::OUT_LEN], n_threads: usize) -> Self {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(n_threads)
            .build()
            .unwrap();
        Mining { site, pool }
    }

    /// Begin mining at site.
    pub fn mine(&self, terminator: Arc<AtomicBool>) -> mpsc::Receiver<MiningRecord> {
        // Record channel
        let (sender, receiver) = mpsc::channel(MINING_RECORD_BUFFER);

        let current_best: RwLock<[u8; blake3::OUT_LEN]> = RwLock::new([0; blake3::OUT_LEN]);

        // Prime hasher with site preimage
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.site);

        self.pool.spawn(move || {
            (0..std::u64::MAX)
                .into_par_iter()
                .map(|i| {
                    if terminator.load(Ordering::Relaxed) {
                        None
                    } else {
                        Some(i)
                    }
                })
                .while_some()
                .for_each(|nonce: u64| {
                    // Add nonce to end of preimage
                    let digest = *hasher
                        .clone()
                        .update(&nonce.to_be_bytes())
                        .finalize()
                        .as_bytes();
                    let current_best_read = current_best.read();
                    if *current_best_read < digest {
                        drop(current_best_read);
                        *current_best.write() = digest;
                        let record = MiningRecord { nonce, digest };
                        sender.clone().try_send(record).expect("mining buffer full");
                    }
                })
        });
        receiver
    }
}
