pub mod codec;

use std::{io, net::SocketAddr, time::Instant};

use futures::channel::{mpsc, oneshot};
use futures::prelude::*;
use futures::stream;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio_util::codec::Framed;

pub use codec::Message;

const DEFAULT_CHANNEL_CAPACITY: usize = 1024;

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

type NewConnectionCallback = oneshot::Sender<Result<(), io::Error>>;

pub struct NewConnection {
    socket: SocketAddr,
    callback: NewConnectionCallback,
}

/// Contains handle to various network resources.
pub struct NetworkHandles {
    /// Add a new connection to event loop
    new_connection_sink: mpsc::Sender<NewConnection>,
    /// Returns framed connections
    conn_stream: Option<mpsc::Receiver<FramedConnection>>,
}

impl NetworkHandles {
    /// Takes connection receiver. Can only be performed once.
    pub fn connection_stream(&mut self) -> Option<mpsc::Receiver<FramedConnection>> {
        self.conn_stream.take()
    }

    pub fn new_connection_sink(&self) -> mpsc::Sender<NewConnection> {
        self.new_connection_sink.clone()
    }
}

impl NetworkHandles {
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

pub struct FramedConnection {
    pub start_time: Instant,
    pub addr: SocketAddr,
    pub framed: Framed<TcpStream, codec::MessageCodec>,
}

impl FramedConnection {
    fn new(addr: SocketAddr, tcp_stream: TcpStream) -> Self {
        let framed = Framed::new(tcp_stream, codec::MessageCodec::default());
        FramedConnection {
            start_time: Instant::now(),
            addr,
            framed,
        }
    }
}

#[derive(Debug)]
pub enum NetworkError {
    MissingBindAddr,
    BindError(io::Error),
}

impl<A: ToSocketAddrs> NetworkBuilder<A> {
    pub async fn start(self) -> Result<NetworkHandles, NetworkError> {
        let listener = TcpListener::bind(self.bind_addr.ok_or(NetworkError::MissingBindAddr)?)
            .await
            .map_err(NetworkError::BindError)?;

        // This channel allows manual connection to peers
        let (new_connection_sink, new_connection_stream) =
            mpsc::channel::<NewConnection>(self.channel_capacity);

        // Once peers connected a new connection is sent
        let (send_conn, recv_conn) = mpsc::channel::<FramedConnection>(self.channel_capacity);

        // Spawn network event loop
        let event_loop = event_loop(listener, new_connection_stream, send_conn);
        tokio::spawn(event_loop);

        Ok(NetworkHandles {
            new_connection_sink,
            conn_stream: Some(recv_conn),
        })
    }
}

async fn event_loop(
    mut listener: TcpListener,
    new_conn_stream: mpsc::Receiver<NewConnection>,
    framed_conn_sink: mpsc::Sender<FramedConnection>,
) {
    // Filter out bad peer connections
    let incoming = listener.incoming().filter_map(|res| async {
        match res {
            Ok(tcp_stream) => match tcp_stream.peer_addr() {
                Ok(addr) => Some((addr, tcp_stream)),
                Err(_err) => None,
            },
            Err(_err) => None, // TODO: Handle error
        }
    });

    // Filter out bad manual connections
    // Trigger callback
    let new_conn_stream = new_conn_stream.filter_map(|new_conn| async {
        match TcpStream::connect(new_conn.socket).await {
            Ok(tcp_stream) => match tcp_stream.peer_addr() {
                Ok(addr) => {
                    new_conn.callback.send(Ok(()));
                    Some((addr, tcp_stream))
                }
                Err(err) => {
                    new_conn.callback.send(Err(err));
                    None
                }
            },
            Err(err) => {
                new_conn.callback.send(Err(err));
                None
            }
        }
    });

    // Combine connection streams and add handles
    let connection_stream = stream::select(incoming, new_conn_stream);

    // Frame streams
    connection_stream
        .for_each(|(addr, tcp_stream)| {
            let mut framed_conn_sink = framed_conn_sink.clone();
            async move {
                let framed_connection = FramedConnection::new(addr, tcp_stream);
                framed_conn_sink
                    .send(framed_connection)
                    .await
                    .expect("dropped framed connection sink");
            }
        })
        .await;
}
