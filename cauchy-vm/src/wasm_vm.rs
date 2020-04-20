use super::{CauchyVM, Result, RetVal, Script, ScriptStatus, VmErr};
use rust_wasm::values::Value;
use rust_wasm::*;
use std::convert::TryFrom;
use std::io::Cursor;

impl TryFrom<Value> for ScriptStatus {
    type Error = VmErr;
    fn try_from(v: Value) -> Result<ScriptStatus> {
        match v {
            Value::I32(v) => Ok(ScriptStatus::try_from(v)?),
            Value::I64(v) => Err(VmErr::BadStatus(v as u32)),
            Value::F32(v) => Err(VmErr::BadStatus(v as u32)),
            Value::F64(v) => Err(VmErr::BadStatus(v as u32)),
        }
    }
}

impl TryFrom<u32> for ScriptStatus {
    type Error = VmErr;
    fn try_from(v: u32) -> Result<ScriptStatus> {
        use ScriptStatus::*;
        match v {
            0x0 => Ok(Ready),
            0xFF => Ok(Completed),
            0xDEADBEEF => Ok(Killed),
            e => Err(VmErr::BadStatus(e)),
        }
    }
}

pub struct WasmVM {}

impl WasmVM {
    pub fn initialize(script: &Script<'_>) -> Result<RetVal> {
        let func = if let Some(func) = &script.func {
            func
        } else {
            "init"
        };
        let store = init_store();
        Self::call_func(script, store, func, None)
    }

    pub fn process_inbox(script: &Script<'_>, message: Option<Vec<u8>>) -> Result<RetVal> {
        let func = if let Some(func) = script.func {
            func
        } else {
            "inbox"
        };
        let mut store = init_store();
        restore_store(&mut store, "some_txid");
        Self::call_func(script, store, func, message)
    }

    fn call_func(
        script: &Script<'_>,
        mut store: Store,
        func: &str,
        message: Option<Vec<u8>>,
    ) -> Result<RetVal> {
        let module = decode_module(Cursor::new(&script.script)).unwrap();
        let module_instance = instantiate_module(&mut store, module, &[]).unwrap();
        if let ExternVal::Func(main_addr) = get_export(&module_instance, func).unwrap() {
            let res = invoke_func(
                &mut store,
                main_addr,
                Vec::new(),
                script.aux_data.as_ref(),
                message.as_ref(),
            );
            println!("func '{}' returned {:X?}", func, res);
            match res {
                Ok(v) if v.0.len() == 1 => {
                    save_store("some_txid", &store);
                    Ok(RetVal {
                        cost: v.1,
                        script_status: ScriptStatus::try_from(v.0[0])?,
                    })
                }
                _ => Err(VmErr::Unknown),
            }
        } else {
            Err(VmErr::Unknown)
        }
    }
}

impl CauchyVM for WasmVM {
    fn initialize(&mut self, script: &Script<'_>) -> Result<RetVal> {
        WasmVM::initialize(script)
    }

    fn process_inbox(&mut self, script: &Script<'_>, message: Option<Vec<u8>>) -> Result<RetVal> {
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
    let dir = dir.join("contracts/contract_data/target/wasm32-unknown-unknown/release/");
    let dir_exists = dir.exists();
    let dir = dir.join("contract_data.wasm");
    let file_exists = dir.exists();

    if dir_exists && !file_exists {
        // Handle the case of the directory existing, meaning the build went OK
        // but the .wasm file does not due to the build environment or some other factor.
        // NOTE: this is to address an issue seen with CI on github
        println!("skipping test due to missing .wasm file");
    } else {
        let mut f = std::fs::File::open(dir.as_path()).expect("failed to open contract_data.wasm");
        let mut script = Vec::new();
        f.read_to_end(&mut script)
            .expect("failed to read contract_data.wasm");

        let aux_data = Some(vec![0xEF, 0xBE, 0xAD, 0xDE, 0x45]);

        let script = Script {
            func: None,
            script,
            aux_data,
        };
        WasmVM::initialize(&script).unwrap();
        WasmVM::process_inbox(&script, None).unwrap();
    }
}
