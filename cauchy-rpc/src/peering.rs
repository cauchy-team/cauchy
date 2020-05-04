pub mod gen {
    tonic::include_proto!("peering");
}

use tokio::net::TcpStream;
use tonic::{Request, Response, Status};
use tower::util::ServiceExt;

use arena::Player;

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
    async fn list_peers(&self, _: Request<()>) -> Result<Response<ListPeersResponse>, Status> {
        let query = arena::player::ArenaQuery(arena::AllQuery(arena::GetMetadata));
        let metadata_map: Result<_, _> = self.player.clone().oneshot(query).await;
        let peer_list = ListPeersResponse {
            peers: metadata_map
                .unwrap()
                .into_iter()
                .map(move |(addr, metadata)| Peer {
                    address: addr.to_string(),
                    start_time: metadata
                        .start_time
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as i64,
                })
                .collect(),
        };
        Ok(Response::new(peer_list))
    }

    async fn connect_peer(&self, request: Request<ConnectRequest>) -> Result<Response<()>, Status> {
        let tcp_stream = TcpStream::connect(request.into_inner().address)
            .await
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        self.player.clone().oneshot(tcp_stream).await; // TODO: Handle

        Ok(Response::new(()))
    }

    async fn disconnect_peer(
        &self,
        _request: Request<DisconnectRequest>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn ban_peer(&self, _request: Request<BanRequest>) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }
}
