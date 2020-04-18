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

#[test]
fn vm_test() {
    use std::io::prelude::*;

    let dir = std::path::PathBuf::new();
    let dir = dir.join("contracts/contract_data/target/wasm32-unknown-unknown/release/");
    let dir_exists = dir.exists();
    let dir = dir.join("contract_data.wasm");
    let file_exists = dir.exists();

    if !file_exists || !dir_exists {
        // If we can't find the file or path, something happened to our build script or it's
        // not working on this platform.  We don't want to fail an entire suite of tests
        // because of a pathing error (experienced during CI)
        println!("failed to fild contract_data.wasm, skipping test");
        for entry in std::fs::read_dir(dir.as_path()).unwrap() {
            println!("{:?}", entry.unwrap());
        }
    } else {
        let mut f = std::fs::File::open(dir.as_path())
            .expect(&format!("failed to open contract_data.wasm at {:?}", dir));
        let mut script = Vec::new();
        f.read_to_end(&mut script)
            .expect(&format!("failed to read contract_data.wasm at {:?}", dir));

        let aux_data = Some(vec![0x41, 0x42, 0x43, 0x44, 0x45]);

        let script = Script {
            func: None,
            script,
            aux_data,
        };
        let res1 = WasmVM::initialize(&script);
        assert!(res1.is_ok());
        let res2 = WasmVM::process_inbox(&script, None);
        assert!(res2.is_ok());
    }
}
