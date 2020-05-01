pub mod gen {
    tonic::include_proto!("peering");
}

use std::io;

use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};
use tonic::{Request, Response, Status};

use gen::peering_server::{Peering, PeeringServer};
use gen::*;

pub struct ConnectMessage {
    pub address: String,
    pub callback: oneshot::Sender<Result<(), io::Error>>,
}

pub struct DisconnectMessage {
    pub address: String,
    pub callback: oneshot::Sender<Result<(), ()>>,
}

pub type ConnectSink = mpsc::Sender<ConnectMessage>;
pub type DisconnectSink = mpsc::Sender<DisconnectMessage>;

#[derive(Clone)]
pub struct PeeringService {
    connect_sink: ConnectSink,
    disconnect_sink: DisconnectSink,
}

impl PeeringService {
    pub fn new(connect_sink: ConnectSink, disconnect_sink: DisconnectSink) -> Self {
        PeeringService {
            connect_sink,
            disconnect_sink,
        }
    }

    pub fn into_server(self) -> PeeringServer<Self> {
        PeeringServer::new(self)
    }
}

#[tonic::async_trait]
impl Peering for PeeringService {
    async fn connect_peer(&self, request: Request<ConnectRequest>) -> Result<Response<()>, Status> {
        let address = request.into_inner().address;
        let (callback, result) = oneshot::channel();

        let new_peer = ConnectMessage { address, callback };
        self.connect_sink
            .clone()
            .send(new_peer)
            .await
            .expect("connect channel dropped");
        result
            .await
            .expect("callback channel dropped")
            .map_err(|err| Status::unavailable(format!("{}", err)))?;

        Ok(Response::new(()))
    }

    async fn disconnect_peer(
        &self,
        request: Request<DisconnectRequest>,
    ) -> Result<Response<()>, Status> {
        let address = request.into_inner().address;
        let (callback, result) = oneshot::channel();

        let new_peer = DisconnectMessage { address, callback };
        self.disconnect_sink
            .clone()
            .send(new_peer)
            .await
            .expect("disconnect channel dropped");
        result
            .await
            .expect("callback channel dropped")
            .map_err(|_| Status::not_found(""))?;

        Ok(Response::new(()))
    }

    async fn ban_peer(&self, request: Request<BanRequest>) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }
}
