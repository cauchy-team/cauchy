use std::process::Command;

fn main() {
    let _output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--target=wasm32-unknown-unknown")
        .current_dir("contracts\\contract_data")
        .output()
        .expect("failed to build contract");

    // Command::new("copy")
    //     .arg("/y")
    //     .arg("\"target\\wasm32-unknown-unknown\\release\\contract_data.wasm\"")
    //     .arg("\"../\"")
    //     .current_dir("contracts\\contract_data")
    //     .output()
    //     .expect("failed to copy contract_data.wasm file :-(");
    println!("cargo:rerun-if-changed=contractscontract_data/src/main.rs");
}
