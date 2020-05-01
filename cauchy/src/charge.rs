use bytes::Bytes;

use arena::{Minisketch, Perception};
use network::codec::Status;
use tokio::sync::RwLock;

use std::sync::Arc;

pub struct Charge {
    inner: Arc<RwLock<ChargeInner>>,
}

#[derive(Clone)]
pub struct ChargeInner {
    oddsketch: Bytes,
    root: Bytes,
    nonce: u64,
    minisketch: Minisketch,
}

impl Charge {
    pub async fn perception(&self) -> Perception {
        Perception {
            root: self.root.clone(),
            minisketch: (),
        }
    }

    pub async fn status(&self) -> Status {
        Status {
            oddsketch: self.oddsketch.clone(),
            root: self.root.clone(),
            nonce: self.nonce,
        }
    }
}
