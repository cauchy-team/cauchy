use parking_lot::RwLock;
use rand::{seq::IteratorRandom, Rng};

use std::{collections::HashMap, sync::Arc};

type OutSocket = (); // Placeholder

pub struct Peer {
    state: PeerState,
    out_socket: OutSocket,
}

#[derive(PartialEq)]
pub enum PeerState {
    NoAuth,
    Idle,
    Polled,
}

pub struct Arena {
    peers: RwLock<HashMap<usize, Arc<RwLock<Peer>>>>,
}

impl Arena {
    pub fn new_peer(&self, out_socket: OutSocket) -> usize {
        let mut rng = rand::thread_rng();

        let index: usize = rng.gen();
        let peer = Peer {
            state: PeerState::NoAuth,
            out_socket,
        };
        let peer = Arc::new(RwLock::new(peer));
        self.peers.write().insert(index, peer);
        index
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

    pub fn poll(&self, n: usize) {
        let chosen_peers = self.choose_multiple(n);
        chosen_peers.into_iter().for_each(|(id, peer)| {
            let mut write_peer = peer.write();
            write_peer.state = PeerState::Polled;
            let socket = write_peer.out_socket;
            drop(write_peer);
            // socket.send()
        })
    }
}
