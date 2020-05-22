use std::{fmt, net::SocketAddr};

/// A `PeerClient` request, sent to the `Arena`. Wraps an `PeerClient` request.
///
/// Sent to a specific `PeerClient` indexed by a `SocketAddr`.
pub struct DirectedQuery<T>(pub SocketAddr, pub T);

/// An error associated with a directed request.
#[derive(Debug)]
pub enum DirectedError<E> {
    Internal(E),
    Missing,
}

impl<E: fmt::Display> fmt::Display for DirectedError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal(err) => err.fmt(f),
            Self::Missing => writeln!(f, "peer not found"),
        }
    }
}

/// A `PeerClient` request, sent to the `Arena`. Wraps an `PeerClient` request.
///
/// Sent to every `PeerClient`.
pub struct AllQuery<T: Sized>(pub T);

/// Sample query message. Wraps an `PeerClient` message.
#[derive(Clone)]
pub struct SampleQuery<T>(pub T, pub usize);

/// An error associated with inserting a peer into the arena.
pub struct InsertPeerError;
