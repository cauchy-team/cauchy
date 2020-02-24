use bytes::Bytes;
use parking_lot::RwLock;

use arena::{Minisketch, Perception};
use network::codec::Status;

use std::sync::Arc;

pub type SharedEgoHandle = Arc<RwLock<EgoHandle>>;

pub struct EgoHandle {
    oddsketch: Bytes,
    root: Bytes,
    nonce: u64,
    minisketch: Minisketch,
}

impl EgoHandle {
    pub fn perception(&self) -> Perception {
        Perception {
            root: self.root.clone(),
            minisketch: (),
        }
    }

    pub fn status(&self) -> Status {
        Status {
            oddsketch: self.oddsketch.clone(),
            root: self.root.clone(),
            nonce: self.nonce,
        }
    }
}
