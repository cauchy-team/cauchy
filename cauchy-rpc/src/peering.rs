pub mod gen {
    tonic::include_proto!("peering");
}

use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};
use tonic::{Request, Response, Status};

use gen::peering_server::{Peering, PeeringServer};
use gen::*;

pub struct NewPeerMessage {
    address: String,
    callback: oneshot::Sender<()>,
}

pub type NewPeerSender = mpsc::Sender<NewPeerMessage>;

#[derive(Clone)]
pub struct PeeringService {
    new_peer_send: NewPeerSender,
}

impl PeeringService {
    pub fn new(new_peer_send: NewPeerSender) -> Self {
        PeeringService { new_peer_send }
    }

    pub fn into_server(self) -> PeeringServer<Self> {
        PeeringServer::new(self)
    }
}

#[tonic::async_trait]
impl Peering for PeeringService {
    async fn connect_peer(
        &self,
        request: Request<ConnectPeerRequest>,
    ) -> Result<Response<()>, Status> {
        let address = request.into_inner().address;
        let (callback, result) = oneshot::channel();

        let new_peer = NewPeerMessage { address, callback };
        self.new_peer_send
            .clone()
            .send(new_peer)
            .await
            .expect("new peer channel dropped");
        result.await;

        Ok(Response::new(()))
    }
}
