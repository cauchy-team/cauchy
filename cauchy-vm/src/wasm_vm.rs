use super::{CauchyVM, RetVal, Script, ScriptStatus};

use common::services::VMError;
use rust_wasm::values::Value;
use rust_wasm::*;
use std::convert::TryFrom;
use std::io::Cursor;

type Result<T> = std::result::Result<T, VMError>;

impl TryFrom<Value> for ScriptStatus {
    type Error = VMError;
    fn try_from(v: Value) -> Result<ScriptStatus> {
        match v {
            Value::I32(v) => Ok(ScriptStatus::try_from(v)?),
            Value::I64(v) => Err(VMError::BadStatus(v as u32)),
            Value::F32(v) => Err(VMError::BadStatus(v as u32)),
            Value::F64(v) => Err(VMError::BadStatus(v as u32)),
        }
    }
}

impl TryFrom<u32> for ScriptStatus {
    type Error = VMError;
    fn try_from(v: u32) -> Result<ScriptStatus> {
        use ScriptStatus::*;
        match v {
            0x0 => Ok(Ready),
            0xFF => Ok(Completed),
            0xDEADBEEF => Ok(Killed),
            e => Err(VMError::BadStatus(e)),
        }
    }
}

#[derive(Clone, Debug)]
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
            // println!("func '{}' returned {:X?}", func, res);
            match res {
                Ok(v) if v.0.len() == 1 => {
                    save_store("some_txid", &store);
                    Ok(RetVal {
                        cost: v.1,
                        script_status: ScriptStatus::try_from(v.0[0])?,
                    })
                }
                _ => Err(VMError::Unknown),
            }
        } else {
            Err(VMError::Unknown)
        }
    }
}

impl CauchyVM for WasmVM {
    fn initialize(script: &Script<'_>) -> Result<RetVal> {
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
