use std::{pin::Pin, sync::Arc};

use futures::{
    prelude::*,
    task::{Context, Poll},
};
use network::{
    codec::{TransactionInv, Transactions},
    Message,
};
// use pin_project::pin_project;
use tokio::sync::RwLock;
use tower::Service;

#[derive(Clone, Default)]
pub struct Database {}

pub enum Error {
    Rocks,
}
