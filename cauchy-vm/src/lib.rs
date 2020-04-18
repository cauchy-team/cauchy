pub mod merkle;
mod wasm_vm;

pub use merkle::*;
pub use wasm_vm::WasmVM as DefaultVM;

pub struct Script<'a> {
    func: Option<&'a str>,
    script: Vec<u8>,
    aux_data: Option<Vec<u8>>,
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

#[cfg(test)]
mod test {
    use crate::{CauchyVM, DefaultVM, Script};
    #[test]
    fn vm_interface() {
        use std::io::prelude::*;

        let dir = std::env::current_dir().unwrap().join(
            "contracts/contract_data/target/wasm32-unknown-unknown/release/contract_data.wasm",
        );
        let mut f = std::fs::File::open(dir.as_path()).expect("failed to open contract_data.wasm");
        let mut script = Vec::new();
        f.read_to_end(&mut script)
            .expect("failed to read contract_data.wasm");
        let aux_data = Some(vec![0x41, 0x42, 0x43, 0x44, 0x45]);
        let script = Script {
            func: None,
            script,
            aux_data,
        };

        let mut vm = DefaultVM::default();
        vm.initialize(&script).unwrap();
        vm.process_inbox(&script, None).unwrap();
    }
}
