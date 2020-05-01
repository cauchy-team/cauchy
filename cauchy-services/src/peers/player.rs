use std::{pin::Pin, sync::Arc};

use futures::{
    prelude::*,
    task::{Context, Poll},
};
use network::{codec::*, FramedStream, Message};
use tokio::sync::RwLock;
use tower::Service;

use super::*;
use crate::database::{Database, Error as DatabaseError};

pub type TowerError = tokio_tower::Error<FramedStream, Message>;

pub type SplitStream = futures::stream::SplitStream<FramedStream>;

#[derive(Clone)]
pub struct Player {
    metadata: Arc<Metadata>,
    database: Database,
    _state_svc: (),
    last_status: Arc<RwLock<Option<Status>>>,
}

impl Player {
    pub fn new(metadata: Arc<Metadata>, database: Database) -> Self {
        Self {
            metadata,
            database,
            _state_svc: (),
            last_status: Default::default(),
        }
    }
}

impl Service<GetStatus> for Player {
    type Response = (Status, Marker);
    type Error = MissingStatus;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetStatus) -> Self::Future {
        let last_status_inner = self.last_status.clone();
        let fut = async move {
            last_status_inner
                .read()
                .await
                .clone()
                .ok_or(MissingStatus)
                .map(move |status| (status, Marker))
        };
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

impl Service<GetMetadata> for Player {
    type Response = Arc<Metadata>;
    type Error = ();
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetMetadata) -> Self::Future {
        let metadata = self.metadata.clone();
        let fut = async move { Ok(metadata) };
        Box::pin(fut)
    }
}
