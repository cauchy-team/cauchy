use std::{net::SocketAddr, time::SystemTime};

pub struct Marker;
pub struct Minisketch;

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
