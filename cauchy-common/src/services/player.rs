use std::{net::SocketAddr, time::SystemTime};

use tokio::net::TcpStream;

use crate::network::Minisketch;
use super::arena::InsertPeerError;

/// Error representing missing status.
#[derive(Debug)]
pub struct MissingStatus;

/// A status request, sent to the `Player` or a `PeerClient`. This gets the cached local status.
pub struct GetStatus;

/// A status request, sent to a `PeerClient`. This refreshes and returns the cached local status.
#[derive(Clone)]
pub struct PollStatus;

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
pub struct MempoolError;

/// An error associated with adding a new peer.
pub enum NewPeerError {
    Network(std::io::Error),
    Arena(InsertPeerError),
}
