pub use blake3;
pub use minisketch_rs::Minisketch as MinisketchCrypto;

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
