pub mod client;
pub mod server;

pub use client::*;
pub use server::*;

use std::{net::SocketAddr, sync::Arc, time::SystemTime};

use futures::{
    channel::mpsc,
    prelude::*,
    task::{Context, Poll},
};
use network::codec::{Status, Transactions};
use network::{codec::*, Message};
use tokio::{
    net::TcpStream,
    sync::{Mutex, RwLock},
};
use tokio_tower::pipeline::Client;
use tokio_util::codec::Framed;
use tower::buffer::Buffer;
use tower::Service;

use super::*;
use crate::player;

pub type TowerError<T> = tokio_tower::Error<T, Message>;

const BUFFER_SIZE: usize = 128;

pub type ClientService =
    Buffer<Client<ClientTransport, TowerError<ClientTransport>, Message>, Message>;

#[derive(Clone)]
pub struct Peer {
    metadata: Arc<Metadata>,
    player: player::Player,
    perception: Arc<Mutex<Option<Marker>>>,
    response_sink: mpsc::Sender<Message>,
    last_status: Arc<RwLock<Option<Status>>>,
    client_svc: ClientService,
}

impl Peer {
    pub fn new(
        player: player::Player,
        tcp_stream: TcpStream,
    ) -> Result<(Self, ServerTransport), std::io::Error> {
        let addr = tcp_stream.peer_addr()?;

        let codec = MessageCodec::default();
        let framed = Framed::new(tcp_stream, codec);

        let (response_sink, response_stream) = mpsc::channel(BUFFER_SIZE);
        let (request_sink, request_stream) = mpsc::channel(BUFFER_SIZE);

        let server_transport = ServerTransport::new(framed, request_stream);

        let client_transport = ClientTransport::new(request_sink, response_stream);
        let client_svc = Buffer::new(Client::new(client_transport), BUFFER_SIZE);

        let metadata = Arc::new(Metadata {
            start_time: SystemTime::now(),
            addr,
        });
        Ok((
            Self {
                metadata,
                player,
                perception: Default::default(),
                response_sink,
                last_status: Default::default(),
                client_svc,
            },
            server_transport,
        ))
    }

    pub fn client_svc(&mut self) -> &mut ClientService {
        &mut self.client_svc
    }
}

pub enum Error {
    ResponseSend(mpsc::SendError),
    MissingStatus(MissingStatus),
    Reconcile(player::ReconcileError),
    Transaction(player::TransactionError),
    GetStatus(MissingStatus),
    TransactionInv(player::TransactionError),
    UnexpectedReconcile,
}

impl Peer {
    pub fn get_metadata(&self) -> Arc<Metadata> {
        self.metadata.clone()
    }

    pub fn get_socket(&self) -> SocketAddr {
        self.metadata.addr.clone()
    }
}
