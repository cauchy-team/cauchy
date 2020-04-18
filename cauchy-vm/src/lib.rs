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

#[derive(Debug)]
pub enum VmErr {
    Unknown,
}

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub trait CauchyVM {
    fn initialize(&mut self, script: &Script<'_>) -> Result<()>;
    fn process_inbox(&mut self, script: &Script<'_>, message: Option<Vec<u8>>) -> Result<()>;
}
