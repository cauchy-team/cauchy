pub mod merkle;
pub use merkle::*;

use std::io::Cursor;

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub trait CauchyVM {
    fn initialize(script: &Script<'_>) -> Result<()>;
    fn process_inbox(script: &Script<'_>, message: Option<Vec<u8>>) -> Result<()>;
}

use rust_wasm::*;

pub struct Script<'a> {
    func: Option<&'a str>,
    script: Vec<u8>,
    aux_data: Option<Vec<u8>>,
}

type Result<T> = std::result::Result<T, VmErr>;

pub enum VmErr {
    Unknown,
}

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
    fn initialize(script: &Script<'_>) -> Result<()> {
        WasmVM::initialize(script)
    }

    fn process_inbox(script: &Script<'_>, message: Option<Vec<u8>>) -> Result<()> {
        WasmVM::process_inbox(script, message)
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
