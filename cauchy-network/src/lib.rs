pub mod codec;

use tokio::net::TcpStream;
use tokio_util::codec::Framed;

pub use codec::Message;

/// Get crate version.
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub type FramedStream = Framed<TcpStream, codec::MessageCodec>;
