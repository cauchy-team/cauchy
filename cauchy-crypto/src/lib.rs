pub use blake3;

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
