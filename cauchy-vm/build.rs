use std::process::Command;

fn main() {
    let cmd = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--target=wasm32-unknown-unknown")
        .current_dir("contracts/contract_data")
        .output()
        .expect("failed to build contract");

    Command::new("cp")
        .arg("target/wasm32-unknown-unknown/release/contract_data.wasm")
        .arg("../../")
        .current_dir("contracts/contract_data")
        .output()
        .expect("failed to copy contract");
}
