pub mod arena;
pub mod database;
pub mod peer;
pub mod player;

use std::{net::SocketAddr, time::SystemTime};

pub struct Marker;
pub struct Minisketch;

pub struct GetStatus;

pub struct MissingStatus;

#[derive(Clone)]
pub struct GetMetadata;

pub struct Metadata {
    pub start_time: SystemTime,
    pub addr: SocketAddr,
}

pub type FutResponse<T, E> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>;
