pub mod gen {
    tonic::include_proto!("info");
}

use tonic::{Request, Response, Status};

use gen::info_server::{Info, InfoServer};
use gen::*;
use std::time::SystemTime;

#[derive(Clone)]
pub struct InfoService {
    daemon_version: String,
    consensus_version: String,
    network_version: String,
    rpc_version: String,
    miner_version: String,
    crypto_version: String,
    start_time: SystemTime,
}

impl InfoService {
    pub fn new(
        daemon_version: String,
        consensus_version: String,
        network_version: String,
        rpc_version: String,
        miner_version: String,
        crypto_version: String,
    ) -> Self {
        InfoService {
            daemon_version,
            consensus_version,
            network_version,
            rpc_version,
            miner_version,
            crypto_version,
            start_time: SystemTime::now(),
        }
    }

    pub fn into_server(self) -> InfoServer<Self> {
        InfoServer::new(self)
    }
}

#[tonic::async_trait]
impl Info for InfoService {
    async fn version(&self, _: Request<()>) -> Result<Response<VersionResponse>, Status> {
        let reply = VersionResponse {
            daemon_version: self.daemon_version.clone(),
            consensus_version: self.consensus_version.clone(),
            network_version: self.network_version.clone(),
            rpc_version: self.rpc_version.clone(),
            miner_version: self.miner_version.clone(),
            crypto_version: self.crypto_version.clone(),
        };
        Ok(Response::new(reply))
    }

    async fn uptime(&self, _: Request<()>) -> Result<Response<UptimeResponse>, Status> {
        let reply = UptimeResponse {
            uptime: SystemTime::now()
                .duration_since(self.start_time)
                .unwrap()
                .as_millis() as u64,
        };
        Ok(Response::new(reply))
    }

    async fn ping(&self, _: Request<()>) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }
}
