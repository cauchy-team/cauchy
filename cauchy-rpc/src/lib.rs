pub mod info;
pub mod peering;

use std::{net::SocketAddr, time::Duration};

use futures::{channel::oneshot, FutureExt};
use tonic::transport::{Error as TransportError, Server};

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[derive(Default)]
pub struct RPCBuilder {
    shutdown_signal: Option<oneshot::Receiver<()>>,
    keep_alive: Option<Duration>,
    info_service: Option<info::InfoService>,
    peering_service: Option<peering::PeeringService>,
}

impl RPCBuilder {
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

    pub fn peering_service(mut self, player: arena::Player) -> Self {
        let peering_service = peering::PeeringService::new(player);
        self.peering_service = Some(peering_service);
        self
    }
}

impl RPCBuilder {
    pub async fn start(self, addr: SocketAddr) -> Result<(), TransportError> {
        let mut builder = Server::builder().tcp_keepalive(self.keep_alive);

        let info_service = self.info_service.expect("info service is required");
        let peering_service = self.peering_service.expect("peer service is required");
        let router = builder
            .add_service(info_service.into_server())
            .add_service(peering_service.into_server());

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
