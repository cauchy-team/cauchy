pub mod gen {
    tonic::include_proto!("peering");
}

use std::io;

use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};
pub use services::player::*;
use tokio::net::TcpStream;
use tonic::{Request, Response, Status};
use tower::Service;

use gen::peering_server::{Peering, PeeringServer};
use gen::*;

#[derive(Clone)]
pub struct PeeringService {
    player: Player,
}

impl PeeringService {
    pub fn new(player: Player) -> Self {
        PeeringService { player }
    }

    pub fn into_server(self) -> PeeringServer<Self> {
        PeeringServer::new(self)
    }
}

#[tonic::async_trait]
impl Peering for PeeringService {
    async fn connect_peer(&self, request: Request<ConnectRequest>) -> Result<Response<()>, Status> {
        let tcp_stream = TcpStream::connect(request.into_inner().address)
            .await
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        self.player.clone().call(tcp_stream).await;

        Ok(Response::new(()))
    }

    async fn disconnect_peer(
        &self,
        request: Request<DisconnectRequest>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn ban_peer(&self, request: Request<BanRequest>) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }
}
