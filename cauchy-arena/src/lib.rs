mod errors;

use parking_lot::RwLock;
use rand::{seq::IteratorRandom, Rng};

use std::collections::hash_map::{Iter, IterMut};
use std::{collections::HashMap, sync::Arc};

use consensus::Entry;
use errors::*;

pub const ROOT_LEN: usize = 32;

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub type Minisketch = ();

pub struct PollResponse {
    entry: Entry,
    root: [u8; ROOT_LEN],
}

pub struct Perception {
    root: [u8; ROOT_LEN],
    minisketch: Minisketch,
}

#[derive(Default)]
pub struct Position {
    state: State,
    poll_response: Option<PollResponse>,
    perception: Option<Perception>,
}

#[derive(PartialEq)]
pub enum State {
    Idle,
    Polled,
}

impl Default for State {
    fn default() -> Self {
        State::Idle
    }
}

pub trait Player {
    fn get_position<'a>(&self) -> &'a Position;

    fn get_mut_position<'a>(&self) -> &'a mut Position;
}

pub trait Arena<K, P>
where
    K: std::cmp::Eq + std::hash::Hash + Clone,
    P: Player,
{
    fn get_player<'a>(&self, key: &K) -> Option<&'a P>;

    fn get_position<'a>(&self, key: &K) -> Option<&'a Position> {
        self.get_player(&key).map(Player::get_position)
    }

    fn players_iter<'a>(&'a self) -> Iter<'a, K, P>;

    fn players_iter_mut<'a>(&'a self) -> IterMut<'a, K, P>;

    fn get_mut_position<'a>(&self, key: &K) -> Option<&'a mut Position> {
        self.get_player(key).map(Player::get_mut_position)
    }

    fn push_perception(&self, key: &K, perception: Perception) {
        self.get_mut_position(key)
            .expect("unexpected id")
            .perception = Some(perception);
    }

    fn choose_multiple<'a>(&self, n: usize) -> Vec<(&K, &P)> {
        let mut rng = rand::thread_rng();
        self.players_iter()
            .filter(|(_, player)| player.get_position().state == State::Idle)
            .choose_multiple(&mut rng, n)
    }

    fn process_poll_response(
        &self,
        key: &K,
        response: PollResponse,
    ) -> Result<(), UnexpectedPollResponse> {
        // Check peer is in polling state
        let position = self.get_mut_position(&key).expect("unexpected id");
        if position.poll_response.is_none() {
            // Add poll response
            position.poll_response = Some(response);
            Ok(())
        } else {
            Err(UnexpectedPollResponse)
        }
    }

    fn conclude_polling(&self) -> PollResults<K> {
        let mut lagards = Vec::with_capacity(256);
        let responses = self
            .players_iter()
            .filter(|(_, player)| player.get_position().state == State::Polled)
            .filter_map(|(key, player)| {
                let position_mut = player.get_mut_position();
                position_mut.state = State::Idle;
                match position_mut.poll_response.take() {
                    Some(entry) => Some((key.clone(), entry)),
                    None => {
                        lagards.push(key.clone());
                        None
                    }
                }
            })
            .collect();
        PollResults { responses, lagards }
    }
}

pub struct PollResults<Key> {
    responses: HashMap<Key, PollResponse>,
    lagards: Vec<Key>,
}

// impl Arena {
//     pub fn new_peer(&self) -> usize {
//         let mut rng = rand::thread_rng();

//         let index: usize = rng.gen();
//         let peer = Peer {
//             state: PeerState::Idle,
//             poll_response: None,
//             perception: None,
//         };
//         let peer = Arc::new(RwLock::new(peer));
//         self.peers.write().insert(index, peer);
//         index
//     }

//     pub fn remove_peer(&self, id: usize) {
//         self.peers.write().remove(&id);
//     }

// }
