use std::{pin::Pin, sync::Arc};

use tower_service::Service;

#[derive(Clone, Default)]
pub struct Database {}

#[derive(Debug)]
pub enum Error {
    Rocks,
}
