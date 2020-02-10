pub mod messages;

use tokio::io::Error;
use tokio::net::{TcpListener, ToSocketAddrs};

/// Contains handle to various network resources.
pub struct NetworkManager {}

impl NetworkManager {
    pub fn build<A>() -> NetworkBuilder<A> {
        NetworkBuilder::default()
    }
}

// Builds the network manager while initializing networking.
pub struct NetworkBuilder<A> {
    bind_addr: Option<A>,
}

impl<A> Default for NetworkBuilder<A> {
    fn default() -> Self {
        NetworkBuilder { bind_addr: None }
    }
}

impl<A> NetworkBuilder<A> {
    pub fn bind(mut self, addr: A) -> Self {
        self.bind_addr = Some(addr);
        self
    }

    pub fn poll_bus(mut self) -> Self {
        self
    }
}

impl<A: ToSocketAddrs> NetworkBuilder<A> {
    pub async fn start(self) -> Result<NetworkManager, Error> {
        let mut listener = TcpListener::bind(self.bind_addr.expect("missing bind address")).await?;

        let event_handle = tokio::spawn(event_loop(listener));

        Ok(NetworkManager {})
    }
}

async fn event_loop(mut listener: TcpListener) -> Result<(), Error> {
    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;
    }
}
