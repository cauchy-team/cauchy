use std::{pin::Pin, sync::Arc};

use futures::{
    prelude::*,
    task::{Context, Poll},
};
use network::{
    codec::{TransactionInv, Transactions},
    Message,
};
use tokio::sync::RwLock;
use tower::Service;

#[derive(Clone, Default)]
pub struct Database {}

#[derive(Debug)]
pub enum Error {
    Rocks,
}
