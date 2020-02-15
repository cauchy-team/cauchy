pub mod codec;

use std::net::SocketAddr;

use futures::channel::mpsc;
use futures::prelude::*;
use futures::stream;
use tokio::io;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio_util::codec::Framed;

pub use codec::Message;

const DEFAULT_CHANNEL_CAPACITY: usize = 1024;

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Contains handle to various network resources.
pub struct NetworkManager {
    send_tcp_stream: mpsc::Sender<TcpStream>,
    conn_stream: Option<mpsc::Receiver<NewConnection>>,
}

impl NetworkManager {
    /// Takes connection receiver. Can only be performed once.
    pub fn connection_stream(&mut self) -> Option<mpsc::Receiver<NewConnection>> {
        self.conn_stream.take()
    }

    /// Get socket stream
    pub fn get_tcp_stream_sender(&self) -> mpsc::Sender<TcpStream> {
        self.send_tcp_stream.clone()
    }

    /// Create a new connection to peer.
    pub async fn new_peer(self, addr: SocketAddr) -> Result<(), io::Error> {
        let tcp_stream = TcpStream::connect(addr).await?;
        self.send_tcp_stream
            .clone()
            .send(tcp_stream)
            .await
            .expect("tcp stream channel dropped");
        Ok(())
    }
}

impl NetworkManager {
    pub fn build<A>() -> NetworkBuilder<A> {
        NetworkBuilder::default()
    }
}

// Builds the network manager while initializing networking.
pub struct NetworkBuilder<A> {
    bind_addr: Option<A>,
    channel_capacity: usize,
}

impl<A> Default for NetworkBuilder<A> {
    fn default() -> Self {
        NetworkBuilder {
            bind_addr: None,
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
        }
    }
}

impl<A> NetworkBuilder<A> {
    pub fn bind(mut self, addr: A) -> Self {
        self.bind_addr = Some(addr);
        self
    }

    pub fn channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }
}

pub struct NewConnection {
    pub addr: SocketAddr,
    pub framed: Framed<TcpStream, codec::MessageCodec>,
}

#[derive(Debug)]
pub enum NetworkError {
    MissingBindAddr,
    BindError(io::Error),
}

impl<A: ToSocketAddrs> NetworkBuilder<A> {
    pub async fn start(self) -> Result<NetworkManager, NetworkError> {
        let listener = TcpListener::bind(self.bind_addr.ok_or(NetworkError::MissingBindAddr)?)
            .await
            .map_err(NetworkError::BindError)?;

        // This channel allows manual connection to peers
        let (send_tcp_stream, recv_tcp_stream) = mpsc::channel::<TcpStream>(self.channel_capacity);

        // Once peers connected a new connection is sent
        let (send_conn, recv_conn) = mpsc::channel::<NewConnection>(self.channel_capacity);

        // Spawn network event loop
        let event_loop = event_loop(listener, recv_tcp_stream, send_conn);
        tokio::spawn(event_loop);

        Ok(NetworkManager {
            send_tcp_stream,
            conn_stream: Some(recv_conn),
        })
    }
}

async fn event_loop(
    mut listener: TcpListener,
    recv_tcp_stream: mpsc::Receiver<TcpStream>,
    send_conn: mpsc::Sender<NewConnection>,
) {
    let incoming = listener.incoming().filter_map(|res| async {
        match res {
            Ok(ok) => Some(ok),
            Err(_err) => None, // TODO: Handle error
        }
    });

    let connection_stream = stream::select(incoming, recv_tcp_stream);
    connection_stream
        .for_each(|tcp_stream| async {
            let handler = frame(tcp_stream, send_conn.clone()).then(|result| async {
                // TODO: Handle error
            });
            tokio::spawn(handler);
        })
        .await;
}

pub enum FramingError {
    Address(io::Error),
    Channel(mpsc::SendError),
}

async fn frame(
    tcp_stream: TcpStream,
    mut send_conn: mpsc::Sender<NewConnection>,
) -> Result<(), FramingError> {
    let addr = tcp_stream.peer_addr().map_err(FramingError::Address)?;

    let framed = Framed::new(tcp_stream, codec::MessageCodec::default());

    let new_connection = NewConnection { addr, framed };
    send_conn
        .send(new_connection)
        .await
        .map_err(FramingError::Channel)?;

    Ok(())
}
