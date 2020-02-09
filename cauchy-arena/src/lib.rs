mod errors;

use parking_lot::RwLock;
use rand::{seq::IteratorRandom, Rng};

use std::{collections::HashMap, sync::Arc};

use consensus::Entry;
use errors::*;

pub const ROOT_LEN: usize = 32;

pub type Minisketch = ();

pub struct PollResponse {
    entry: Entry,
    root: [u8; ROOT_LEN],
}

pub struct Perception {
    root: [u8; ROOT_LEN],
    minisketch: Minisketch,
}

pub struct Peer {
    state: PeerState,
    poll_response: Option<PollResponse>,
    perception: Option<Perception>,
}

#[derive(PartialEq)]
pub enum PeerState {
    Idle,
    Polled,
}

#[derive(Default)]
pub struct Arena {
    peers: RwLock<HashMap<usize, Arc<RwLock<Peer>>>>,
}

pub struct PollResults {
    responses: HashMap<usize, PollResponse>,
    lagards: Vec<usize>,
}

impl Arena {
    pub fn new_peer(&self) -> usize {
        let mut rng = rand::thread_rng();

        let index: usize = rng.gen();
        let peer = Peer {
            state: PeerState::Idle,
            poll_response: None,
            perception: None,
        };
        let peer = Arc::new(RwLock::new(peer));
        self.peers.write().insert(index, peer);
        index
    }

    pub fn push_perception(&self, id: usize, perception: Perception) {
        self.peers
            .read()
            .get(&id)
            .expect("unexpected id")
            .write()
            .perception = Some(perception);
    }

    pub fn remove_peer(&self, id: usize) {
        self.peers.write().remove(&id);
    }

    pub fn process_poll_response(
        &self,
        id: usize,
        response: PollResponse,
    ) -> Result<(), UnexpectedPollResponse> {
        // Check peer is in polling state
        let peers_read = self.peers.read();
        let peer = peers_read.get(&id).expect("unexpected id");
        let mut peer_write = peer.write();
        if peer_write.poll_response.is_none() {
            // Add poll response
            peer_write.poll_response = Some(response);
            Ok(())
        } else {
            Err(UnexpectedPollResponse)
        }
    }

    pub fn conclude_polling(&self) -> PollResults {
        let peers_read = self.peers.read();
        let mut lagards = Vec::with_capacity(256);
        let responses = peers_read
            .iter()
            .filter(|(_, peer)| peer.read().state == PeerState::Polled)
            .filter_map(|(id, peer)| {
                let mut peer_write = peer.write();
                peer_write.state = PeerState::Idle;
                match peer_write.poll_response.take() {
                    Some(entry) => Some((*id, entry)),
                    None => {
                        lagards.push(*id);
                        None
                    }
                }
            })
            .collect();
        PollResults { responses, lagards }
    }

    pub fn choose_multiple(&self, n: usize) -> Vec<(usize, Arc<RwLock<Peer>>)> {
        let mut rng = rand::thread_rng();
        self.peers
            .read()
            .iter()
            .filter(|(_, peer)| {
                let read_peer = peer.read();
                let res = read_peer.state == PeerState::Idle;
                drop(read_peer);
                res
            })
            .choose_multiple(&mut rng, n)
            .into_iter()
            .map(move |(id, peer)| (*id, peer.clone()))
            .collect()
    }
}
