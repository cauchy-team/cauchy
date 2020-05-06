pub mod messages;

use std::{net::SocketAddr, time::SystemTime};

pub use messages::*;

#[derive(Debug)]
pub struct Marker;

pub struct GetStatus;

#[derive(Debug)]
pub struct MissingStatus;

#[derive(Clone)]
pub struct GetMetadata;

pub type FutResponse<T, E> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>;

pub struct Metadata {
    pub start_time: SystemTime,
    pub addr: SocketAddr,
}

/// Directed query message. Wraps an `PeerClient` message.
pub struct DirectedQuery<T>(pub SocketAddr, pub T);

/// All query message. Wraps an `PeerClient` message.
pub struct AllQuery<T: Sized>(pub T);

/// Sample query message. Wraps an `PeerClient` message.
#[derive(Clone)]
pub struct SampleQuery<T>(pub T, pub usize);

//// Poll a peers status.
#[derive(Clone)]
pub struct PollStatus;

pub struct Reconcile(pub Minisketch);
