mod contract_storage;
pub mod merkle;
mod wasm_vm;

pub use merkle::*;
pub use wasm_vm::WasmVM as DefaultVM;

pub struct Script<'a> {
    pub func: Option<&'a str>,
    pub script: Vec<u8>,
    pub aux_data: Option<Vec<u8>>,
}

type Result<T> = std::result::Result<T, VmErr>;

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

#[derive(Debug)]
pub enum VmErr {
    BadStatus(u32),
    Unknown,
}

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub trait CauchyVM {
    fn initialize(&mut self, script: &Script<'_>) -> Result<RetVal>;
    fn process_inbox(&mut self, script: &Script<'_>, message: Option<Vec<u8>>) -> Result<RetVal>;
}
