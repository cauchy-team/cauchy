use bytes::Bytes;

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub trait CauchyVM {
    fn initialize(transaction: Bytes) -> Result<(), ()>;
    fn process_inbox(transaction: Bytes, message: Bytes) -> Result<(), ()>;
}

use rust_wasm::*;
use std::io::BufReader;

pub struct WasmVM {}

impl WasmVM {
    pub fn initialize(_transaction: Bytes) -> Result<(), ()> {
        // TODO: get data from transaction
        let contract_data = Some(vec![0x41, 0x42, 0x43, 0x44, 0x45]);
        let f = std::fs::File::open("contract_data.wasm").unwrap();
        let module = decode_module(BufReader::new(f)).unwrap();

        let mut store = init_store();
        let module_instance = instantiate_module(&mut store, module, &[]).unwrap();
        if let ExternVal::Func(main_addr) = get_export(&module_instance, "app_init").unwrap() {
            let res = invoke_func(&mut store, main_addr, Vec::new(), contract_data);
            println!("{:X?}", res);
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn process_inbox(_transaction: Bytes, message: Bytes) -> Result<(), ()> {
        Ok(())
    }
}

impl CauchyVM for WasmVM {
    fn initialize(transaction: Bytes) -> Result<(), ()> {
        WasmVM::initialize(transaction)
    }

    fn process_inbox(transaction: Bytes, message: Bytes) -> Result<(), ()> {
        WasmVM::process_inbox(transaction, message)
    }
}

#[test]
fn vm_test() {
    let res = WasmVM::initialize(Bytes::new());
    assert!(res.is_ok());
}
