use std::net::SocketAddr;

/// A `PeerClient` request, sent to the `Arena`. Wraps an `PeerClient` request.
///
/// Sent to a specific `PeerClient` indexed by a `SocketAddr`.
pub struct DirectedQuery<T>(pub SocketAddr, pub T);

/// A `PeerClient` request, sent to the `Arena`. Wraps an `PeerClient` request.
///
/// Sent to every `PeerClient`.
pub struct AllQuery<T: Sized>(pub T);

/// Sample query message. Wraps an `PeerClient` message.
#[derive(Clone)]
pub struct SampleQuery<T>(pub T, pub usize);

/// An error associated with inserting a peer into the arena.
pub struct InsertPeerError;
