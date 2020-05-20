pub mod merkle;

pub use blake3;
pub use minisketch_rs::{Minisketch, MinisketchError};
pub use oddsketch::Oddsketch;

/// Get crate version.
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
