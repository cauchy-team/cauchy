use std::{fmt, net::SocketAddr, time::SystemTime};

use tokio::net::TcpStream;

use super::{arena::InsertPeerError, vm::VMSpawnError};
use crate::network::Minisketch;

/// Error representing missing status.
#[derive(Debug)]
pub struct MissingStatus;

/// A status request, sent to the `Player` or a `PeerClient`. This gets the cached local status.
pub struct GetStatus;

/// A status request, sent to a `PeerClient`. This refreshes and returns the cached local status.
#[derive(Clone)]
pub struct PollStatus;

/// An error encountered while calling `PollStatus`.
pub enum PollStatusError {
    /// Peer responded with an unexpected response.
    UnexpectedResponse,
    /// Server error.
    Tower(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for PollStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedResponse => writeln!(f, "unexpected response"),
            Self::Tower(err) => err.fmt(f),
        }
    }
}

/// A reconciliation request, sent to a `PeerClient`. This initiates the reconciliation round-trip.
pub struct Reconcile(pub Minisketch);

/// A players or peers metadata.
pub struct Metadata {
    pub start_time: SystemTime,
    pub addr: SocketAddr,
}

/// A metadata request, sent to the `Player` or a `PeerClient`.
#[derive(Clone)]
pub struct GetMetadata;

/// An `Arena` request, sent to the `Player`. Wraps an `Arena` request.
pub struct ArenaQuery<T>(pub T);

/// A new peer request, sent to the `Player`.
pub struct NewPeer(pub TcpStream);

/// A remove peer request, sent to the `Player`.
pub struct RemovePeer(pub SocketAddr);

/// An error associated with inserting a transaction into the mempool.
pub enum MempoolError {
    VM(VMSpawnError),
}

/// An error associated with adding a new peer.
pub enum NewPeerError {
    Network(std::io::Error),
    Arena(InsertPeerError),
}
