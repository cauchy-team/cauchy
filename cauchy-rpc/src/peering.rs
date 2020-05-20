pub mod gen {
    tonic::include_proto!("peering");
}

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::net::TcpStream;
use tonic::{Request, Response};
use tower_service::Service;
use tower_util::ServiceExt;
use tracing::info;

use common::{network::Status, services::*};

use gen::peering_server::Peering;
use gen::*;

#[derive(Clone)]
pub struct PeeringService<Pl> {
    player: Pl,
}

impl<Pl> PeeringService<Pl> {
    pub fn new(player: Pl) -> Self {
        PeeringService { player }
    }
}

#[tonic::async_trait]
impl<Pl> Peering for PeeringService<Pl>
where
    Pl: Clone + Send + Sync + 'static,
    // Get all metadata
    Pl: Service<ArenaQuery<AllQuery<GetMetadata>>, Response = HashMap<SocketAddr, Arc<Metadata>>>,
    <Pl as Service<ArenaQuery<AllQuery<GetMetadata>>>>::Future: Send,
    <Pl as Service<ArenaQuery<AllQuery<GetMetadata>>>>::Error: std::fmt::Debug,
    // Add new peers
    Pl: Service<NewPeer, Error = NewPeerError>,
    <Pl as Service<NewPeer>>::Response: Send,
    <Pl as Service<NewPeer>>::Future: Send,
    // Remove peers
    Pl: Service<RemovePeer>,
    <Pl as Service<RemovePeer>>::Response: Send,
    <Pl as Service<RemovePeer>>::Future: Send,
    // Poll peer
    Pl: Service<ArenaQuery<DirectedQuery<PollStatus>>, Response = Status>,
    <Pl as Service<ArenaQuery<DirectedQuery<PollStatus>>>>::Future: Send,
{
    async fn list_peers(
        &self,
        _: Request<()>,
    ) -> Result<Response<ListPeersResponse>, tonic::Status> {
        let query = ArenaQuery(AllQuery(GetMetadata));
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

    async fn poll(
        &self,
        request: Request<PollRequest>,
    ) -> Result<Response<PollResponse>, tonic::Status> {
        let addr: std::net::SocketAddr = request
            .into_inner()
            .address
            .parse()
            .map_err(|err| tonic::Status::invalid_argument(format!("{}", err)))?;
        let query = ArenaQuery(DirectedQuery(addr, PollStatus));
        let player = self.player.clone();
        let status = player
            .oneshot(query)
            .await
            .map_err(|_| tonic::Status::unavailable("todo display for this error"))?;

        let poll_response = PollResponse {
            oddsketch: status.oddsketch.to_vec(),
            root: status.root.to_vec(),
            nonce: status.nonce,
        };
        info!("poll response {:?}", poll_response);
        Ok(Response::new(poll_response))
    }

    async fn connect_peer(
        &self,
        request: Request<ConnectRequest>,
    ) -> Result<Response<()>, tonic::Status> {
        let tcp_stream = TcpStream::connect(request.into_inner().address)
            .await
            .map_err(|err| tonic::Status::invalid_argument(err.to_string()))?;
        self.player
            .clone()
            .oneshot(NewPeer(tcp_stream))
            .await
            .map_err(|err| match err {
                NewPeerError::Arena(_err) => tonic::Status::failed_precondition("maximum peers"),
                NewPeerError::Network(err) => tonic::Status::invalid_argument(err.to_string()),
            })?;

        Ok(Response::new(()))
    }

    async fn disconnect_peer(
        &self,
        request: Request<DisconnectRequest>,
    ) -> Result<Response<()>, tonic::Status> {
        let socket_addr = request
            .into_inner()
            .address
            .parse::<SocketAddr>()
            .map_err(move |err| tonic::Status::invalid_argument(err.to_string()))?;

        self.player
            .clone()
            .oneshot(RemovePeer(socket_addr))
            .await
            .map(|_| Response::new(()))
            .map_err(|_| tonic::Status::not_found("peer not found"))
    }

    async fn ban_peer(&self, _request: Request<BanRequest>) -> Result<Response<()>, tonic::Status> {
        Ok(Response::new(()))
    }
}
