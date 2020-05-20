use std::{net::SocketAddr, time::SystemTime};

use tokio::net::TcpStream;

use crate::network::Minisketch;

pub struct GetStatus;

//// Poll a peers status.
#[derive(Clone)]
pub struct PollStatus;

pub struct Reconcile(pub Minisketch);

pub struct Metadata {
    pub start_time: SystemTime,
    pub addr: SocketAddr,
}

#[derive(Debug)]
pub struct MissingStatus;

#[derive(Clone)]
pub struct GetMetadata;

/// Query arena.
pub struct ArenaQuery<T>(pub T);

/// New peer.
pub struct NewPeer(pub TcpStream);
