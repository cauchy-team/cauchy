#[cfg(test)]
mod vm_tests {
    use cauchy_vm::{DefaultVM, Script, ScriptStatus};
    use std::include_bytes;

    const BASIC_WASM: &[u8] = include_bytes!("contract_data.wasm");

    #[test]
    fn vm_interface() {
        let aux_data = Some(vec![0xEF, 0xBE, 0xAD, 0xDE, 0x45]);

        let script = Script {
            func: None,
            script: Vec::from(BASIC_WASM),
            aux_data,
        };
        let res = DefaultVM::initialize(&script).unwrap();
        assert_eq!(res.status(), &ScriptStatus::Killed);
        let res = DefaultVM::process_inbox(&script, None).unwrap();
        assert_eq!(res.status(), &ScriptStatus::Completed);
    }
}
