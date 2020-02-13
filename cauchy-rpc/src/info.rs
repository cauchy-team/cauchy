pub mod gen {
    tonic::include_proto!("info");
}

use tonic::{Request, Response, Status};

use gen::info_server::{Info, InfoServer};
use gen::*;
use std::time::Instant;

#[derive(Clone)]
pub struct InfoService {
    daemon_version: String,
    start_time: Instant,
}

impl InfoService {
    pub fn new(daemon_version: String) -> Self {
        InfoService {
            daemon_version,
            start_time: Instant::now(),
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
        };
        Ok(Response::new(reply))
    }

    async fn uptime(&self, _: Request<()>) -> Result<Response<UptimeResponse>, Status> {
        let reply = UptimeResponse {
            uptime: Instant::now().duration_since(self.start_time).as_millis() as u64,
        };
        Ok(Response::new(reply))
    }

    async fn ping(&self, _: Request<()>) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }
}
