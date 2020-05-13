use std::net::SocketAddr;

/// Directed query message. Wraps an `PeerClient` message.
pub struct DirectedQuery<T>(pub SocketAddr, pub T);

/// All query message. Wraps an `PeerClient` message.
pub struct AllQuery<T: Sized>(pub T);

/// Sample query message. Wraps an `PeerClient` message.
#[derive(Clone)]
pub struct SampleQuery<T>(pub T, pub usize);
