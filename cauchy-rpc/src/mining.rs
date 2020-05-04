pub mod gen {
    tonic::include_proto!("mining");
}

use std::sync::atomic::Ordering;

use tonic::{Request, Response, Status};

use gen::mining_server::{Mining, MiningServer};
use gen::*;

#[derive(Clone)]
pub struct MiningService {
    coordinator: miner::MiningCoordinator,
}

impl MiningService {
    pub fn new(coordinator: miner::MiningCoordinator) -> Self {
        MiningService { coordinator }
    }

    pub fn into_server(self) -> MiningServer<Self> {
        MiningServer::new(self)
    }
}

#[tonic::async_trait]
impl Mining for MiningService {
    async fn mining_info(&self, _: Request<()>) -> Result<Response<MiningInfoResponse>, Status> {
        let n_workers = self.coordinator.n_workers();
        let info = match self.coordinator.current_miner().await {
            Some(miner) => MiningInfoResponse {
                n_workers,
                site: miner.site.to_vec(),
                best_nonce: miner.best_nonce.load(Ordering::SeqCst),
                best_digest: miner.best_digest.lock().to_vec(),
            },
            None => MiningInfoResponse {
                n_workers,
                site: vec![],
                best_nonce: 0,
                best_digest: vec![],
            },
        };
        Ok(Response::new(info))
    }
}
