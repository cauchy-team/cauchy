mod wasm_vm;

use common::{
    network::Transaction,
    services::{VMError, VMSpawnError},
    FutResponse,
};

pub use crypto::merkle::*;
use futures_core::task::{Context, Poll};
use tower_service::Service;
pub use wasm_vm::WasmVM as DefaultVM;

pub struct Script<'a> {
    pub func: Option<&'a str>,
    pub script: Vec<u8>,
    pub aux_data: Option<Vec<u8>>,
}

impl<'a> From<Transaction> for Script<'a> {
    fn from(tx: Transaction) -> Script<'a> {
        Script {
            func: None,
            script: tx.binary.to_vec(),
            aux_data: None,
        }
    }
}

// type Result<T> = std::result::Result<T, VMError>;

#[derive(Debug, PartialEq)]
pub enum ScriptStatus {
    Ready = 0x0,
    Completed = 0xFF,
    Killed = 0xDEADBEEF,
}

pub struct RetVal {
    cost: u128,
    script_status: ScriptStatus,
}

impl RetVal {
    pub fn status(&self) -> &ScriptStatus {
        &self.script_status
    }
    pub fn cost(&self) -> u128 {
        self.cost
    }
}

/// Get crate version.
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub trait CauchyVM {
    fn initialize(script: &Script<'_>) -> Result<RetVal, VMError>;
    fn process_inbox(
        &mut self,
        script: &Script<'_>,
        message: Option<Vec<u8>>,
    ) -> Result<RetVal, VMError>;
}

#[derive(Clone, Default)]
pub struct VMFactory<V> {
    _vm: std::marker::PhantomData<V>,
}

impl<V: CauchyVM> Service<Transaction> for VMFactory<V> {
    type Response = RetVal;
    type Error = VMSpawnError;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, tx: Transaction) -> Self::Future {
        let new_script = Script::from(tx);
        let fut = async move { V::initialize(&new_script).map_err(VMSpawnError::Spawn) };
        Box::pin(fut)
    }
}

/*
Transaction arrives -> Spawns a VMFactory from singleton VMFactoryFactory?
Does some shit, uses the spawned VMFactory to spawn actors.

Handle backpressure at the level of the VMFactoryFactory AND VMFactory


Peer - tx -> (handle backpressure) Player - ??? -> VMFactoryFactory
*/
