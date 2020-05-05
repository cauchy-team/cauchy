pub mod client;
pub mod server;

pub use client::*;
pub use server::*;

use std::{net::SocketAddr, sync::Arc, time::SystemTime};

use futures::channel::mpsc;
use network::codec::Status;
use network::{codec::*, Message};
use tokio::{
    net::TcpStream,
    sync::{Mutex, RwLock},
};
use tokio_tower::pipeline::Client;
use tokio_util::codec::Framed;
use tower::buffer::Buffer;

use super::*;
use crate::player;

pub type TowerError<T> = tokio_tower::Error<T, Message>;

pub const BUFFER_SIZE: usize = 128;

#[derive(Clone)]
pub struct Peer<Pl> {
    pub player: Pl,
    pub perception: Arc<Mutex<Option<Marker>>>,
    pub response_sink: mpsc::Sender<Message>,
}

pub trait PeerMetadata {
    fn get_metadata(&self) -> Arc<Metadata>;
    fn get_socket(&self) -> SocketAddr;
}

pub enum Error {
    ResponseSend(mpsc::SendError),
    MissingStatus(MissingStatus),
    Reconcile(player::ReconcileError),
    Transaction(player::TransactionError),
    GetStatus(MissingStatus),
    TransactionInv(player::TransactionError),
    UnexpectedReconcile,
}
