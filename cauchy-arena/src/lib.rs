mod errors;

use bytes::Bytes;
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
    pub entry: Entry,
    pub root: [u8; ROOT_LEN],
}

pub struct Perception {
    pub root: Bytes,
    pub minisketch: Minisketch,
}

#[derive(Default)]
pub struct Position {
    pub state: State,
    pub poll_response: Option<PollResponse>,
    pub perception: Option<Perception>,
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
    fn get_position(&self) -> Arc<RwLock<Position>>;
}

pub trait Arena<K, P>
where
    K: std::cmp::Eq + std::hash::Hash + Clone,
    P: Player + Clone,
{
    fn get_player(&self, key: &K) -> Option<P>;

    fn players_map(&self) -> Arc<RwLock<HashMap<K, P>>>;

    fn choose_multiple(&self, n: usize) -> Vec<(K, P)> {
        let mut rng = rand::thread_rng();
        self.players_map()
            .read()
            .iter()
            .filter(|(_, player)| player.get_position().read().state == State::Idle)
            // .cloned()
            .map(move |(key, player)| (key.clone(), player.clone()))
            .choose_multiple(&mut rng, n)
    }

    fn process_poll_response(
        &self,
        key: &K,
        response: PollResponse,
    ) -> Result<(), UnexpectedPollResponse> {
        // Check peer is in polling state
        let position = self.get_player(&key).expect("unexpected id").get_position();
        if position.read().poll_response.is_none() {
            // Add poll response
            position.write().poll_response = Some(response);
            Ok(())
        } else {
            Err(UnexpectedPollResponse)
        }
    }

    fn conclude_polling<I: Iterator<Item = (K, P)>>(&self) -> PollResults<K> {
        let mut lagards = Vec::with_capacity(256);
        let responses = self
            .players_map()
            .read()
            .iter()
            .filter(|(_, player)| player.get_position().read().state == State::Polled)
            .filter_map(|(key, player)| {
                let position = player.get_position();
                let mut position_mut = position.write();
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
