use std::{net::SocketAddr, pin::Pin, sync::Arc, time::Instant};

use futures::{
    prelude::*,
    task::{Context, Poll},
};
use network::{codec::*, FramedStream, Message};
// use pin_project::pin_project;
use tokio::sync::RwLock;
use tower_service::Service;

use super::*;
use crate::database::{Database, Error as DatabaseError};

pub type TowerError = tokio_tower::Error<FramedStream, Message>;

pub type SplitStream = futures::stream::SplitStream<FramedStream>;

#[derive(Clone)]
pub struct Player {
    start_time: Instant,
    addr: SocketAddr,
    database: Database,
    _state_svc: (),
    last_status: Arc<RwLock<Option<Status>>>,
}

impl Player {
    pub fn new(addr: SocketAddr, database: Database) -> Self {
        Self {
            start_time: Instant::now(),
            database,
            addr,
            _state_svc: (),
            last_status: Default::default(),
        }
    }
}

impl Service<GetStatus> for Player {
    type Response = Status;
    type Error = MissingStatus;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetStatus) -> Self::Future {
        let last_status_inner = self.last_status.clone();
        let fut = async move { last_status_inner.read().await.clone().ok_or(MissingStatus) };
        Box::pin(fut)
    }
}

pub enum TransactionError {
    Database(DatabaseError),
}

impl Service<TransactionInv> for Player {
    type Response = Transactions;
    type Error = TransactionError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO: Check for database locks
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, inv: TransactionInv) -> Self::Future {
        unimplemented!()
    }
}

pub struct Metadata {
    start_time: Instant,
    addr: SocketAddr,
}

impl Service<GetMetadata> for Player {
    type Response = Metadata;
    type Error = ();
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetMetadata) -> Self::Future {
        let start_time = self.start_time.clone();
        let addr = self.addr.clone();
        let fut = async move { Ok(Metadata { start_time, addr }) };
        Box::pin(fut)
    }
}
