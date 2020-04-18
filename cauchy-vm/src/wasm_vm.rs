use super::{CauchyVM, Result, Script, VmErr};
use rust_wasm::*;
use std::io::Cursor;

pub struct WasmVM {}

impl WasmVM {
    pub fn initialize(script: &Script<'_>) -> Result<()> {
        let func = if let Some(func) = &script.func {
            func
        } else {
            "init"
        };

        println!("{:X?}", &script.aux_data);

        let module = decode_module(Cursor::new(&script.script)).unwrap();
        let mut store = init_store();
        let module_instance = instantiate_module(&mut store, module, &[]).unwrap();
        if let ExternVal::Func(main_addr) = get_export(&module_instance, func).unwrap() {
            let res = invoke_func(
                &mut store,
                main_addr,
                Vec::new(),
                script.aux_data.as_ref(),
                None,
            );
            println!("{:X?}", res);
            save_store("some_txid", &store);
            match res {
                Ok(_) => Ok(()),
                Err(_) => Err(VmErr::Unknown),
            }
        } else {
            Err(VmErr::Unknown)
        }
    }

    pub fn process_inbox(script: &Script<'_>, message: Option<Vec<u8>>) -> Result<()> {
        let module = decode_module(Cursor::new(&script.script)).unwrap();
        let mut store = init_store();
        let func = if let Some(func) = script.func {
            func
        } else {
            "inbox"
        };
        let module_instance = instantiate_module(&mut store, module, &[]).unwrap();
        restore_store(&mut store, "some_txid");
        if let ExternVal::Func(main_addr) = get_export(&module_instance, func).unwrap() {
            let res = invoke_func(
                &mut store,
                main_addr,
                Vec::new(),
                script.aux_data.as_ref(),
                message.as_ref(),
            );
            println!("{:X?}", res);
            save_store("some_txid", &store);
            match res {
                Ok(_) => Ok(()),
                Err(_) => Err(VmErr::Unknown),
            }
        } else {
            Err(VmErr::Unknown)
        }
    }
}

impl CauchyVM for WasmVM {
    fn initialize(&mut self, script: &Script<'_>) -> Result<()> {
        WasmVM::initialize(script)
    }

    fn process_inbox(&mut self, script: &Script<'_>, message: Option<Vec<u8>>) -> Result<()> {
        WasmVM::process_inbox(script, message)
    }
}

impl Default for WasmVM {
    fn default() -> Self {
        WasmVM {}
    }
}

#[test]
fn vm_test() {
    use std::env;
    use std::io::prelude::*;

    let dir = env::current_dir().unwrap();
    let dir = dir
        .join("contracts/contract_data/target/wasm32-unknown-unknown/release/contract_data.wasm");

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
    WasmVM::initialize(&script).unwrap();
    WasmVM::process_inbox(&script, None).unwrap();
}
