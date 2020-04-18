use std::process::Command;

fn main() {
    let _output = Command::new("cargo")
        .current_dir("contracts/contract_data")
        .arg("build")
        .arg("--release")
        .arg("--target=wasm32-unknown-unknown")
        .output()
        .expect("failed to build contract");
}
