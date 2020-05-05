pub mod info;
pub mod mining;
pub mod peering;

use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use arena::{
    arena::{AllQuery, DirectedQuery},
    player::ArenaQuery,
};
use futures::{channel::oneshot, FutureExt};
use tokio::net::TcpStream;
use tonic::transport::{Error as TransportError, Server};

use tower::Service;

use peering::gen::peering_server::PeeringServer;

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub struct RPCBuilder<Pl> {
    shutdown_signal: Option<oneshot::Receiver<()>>,
    keep_alive: Option<Duration>,
    info_service: Option<info::InfoService>,
    peering_service: Option<peering::PeeringService<Pl>>,
    mining_service: Option<mining::MiningService>,
}

impl<Pl> Default for RPCBuilder<Pl> {
    fn default() -> Self {
        Self {
            shutdown_signal: None,
            keep_alive: None,
            info_service: None,
            peering_service: None,
            mining_service: None,
        }
    }
}

impl<Pl> RPCBuilder<Pl> {
    pub fn shutdown_signal(mut self, recv: oneshot::Receiver<()>) -> Self {
        self.shutdown_signal = Some(recv);
        self
    }

    pub fn keep_alive(mut self, duration: Duration) -> Self {
        self.keep_alive = Some(duration);
        self
    }

    pub fn info_service(
        mut self,
        daemon_version: String,
        consensus_version: String,
        network_version: String,
        rpc_version: String,
        miner_version: String,
        crypto_version: String,
    ) -> Self {
        let info_service = info::InfoService::new(
            daemon_version,
            consensus_version,
            network_version,
            rpc_version,
            miner_version,
            crypto_version,
        );
        self.info_service = Some(info_service);
        self
    }

    pub fn peering_service(mut self, player: Pl) -> Self {
        let peering_service = peering::PeeringService::new(player);
        self.peering_service = Some(peering_service);
        self
    }

    pub fn mining_service(mut self, coordinator: miner::MiningCoordinator) -> Self {
        let mining_service = mining::MiningService::new(coordinator);
        self.mining_service = Some(mining_service);
        self
    }
}

impl<Pl> RPCBuilder<Pl>
where
    Pl: Clone + Send + Sync + 'static,
    // Get all metadata
    Pl: Service<
        ArenaQuery<AllQuery<arena::GetMetadata>>,
        Response = HashMap<SocketAddr, Arc<arena::Metadata>>,
    >,
    <Pl as Service<ArenaQuery<AllQuery<arena::GetMetadata>>>>::Error: std::fmt::Debug,
    <Pl as Service<ArenaQuery<AllQuery<arena::GetMetadata>>>>::Future: Send,
    // Add new peers
    Pl: Service<TcpStream>,
    <Pl as Service<TcpStream>>::Response: Send,
    <Pl as Service<TcpStream>>::Future: Send,
    // Poll peer
    Pl: Service<
        ArenaQuery<DirectedQuery<arena::peer::PollStatus>>,
        Response = network::codec::Status,
    >,
    <Pl as Service<ArenaQuery<DirectedQuery<arena::peer::PollStatus>>>>::Future: Send,
{
    pub async fn start(self, addr: SocketAddr) -> Result<(), TransportError> {
        let mut builder = Server::builder().tcp_keepalive(self.keep_alive);

        let info_service = self.info_service.expect("info service is required");
        let peering_service = self.peering_service.expect("peer service is required");
        let mining_service = self.mining_service.expect("mining service is required");
        let router = builder
            .add_service(info_service.into_server())
            .add_service(PeeringServer::new(peering_service))
            .add_service(mining_service.into_server());

        if let Some(shutdown_signal) = self.shutdown_signal {
            router
                .serve_with_shutdown(
                    addr,
                    shutdown_signal.map(|res| res.expect("rpc shutdown channel dropped")),
                )
                .await
        } else {
            router.serve(addr).await
        }
    }
}
