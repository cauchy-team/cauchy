pub mod client;
pub mod server;

pub use client::*;
pub use server::*;

use std::{net::SocketAddr, sync::Arc};

use futures_channel::mpsc;
use tokio::sync::{Mutex, RwLock};
use tokio_tower::pipeline::Client;
use tower_buffer::Buffer;

use common::{Metadata, Minisketch};
use network::Message;

pub type TowerError<T> = tokio_tower::Error<T, Message>;

pub const BUFFER_SIZE: usize = 128;

#[derive(Clone)]
pub struct Peer<Pl> {
    pub player: Pl,
    pub perception: Arc<Mutex<Option<Minisketch>>>,
    pub response_sink: mpsc::Sender<Message>,
}

pub trait PeerMetadata {
    fn get_metadata(&self) -> Arc<Metadata>;
    fn get_socket(&self) -> SocketAddr;
}
