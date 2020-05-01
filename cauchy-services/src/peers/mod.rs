pub mod client;
pub mod server;

use std::{net::SocketAddr, time::Instant};

pub struct Marker;
pub struct Minisketch;

pub struct GetStatus;

pub struct MissingStatus;

pub struct GetMetadata;

pub struct Metadata {
    pub start_time: Instant,
    pub addr: SocketAddr,
}
