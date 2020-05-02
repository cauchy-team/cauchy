use std::{net::SocketAddr, sync::Arc};

use tower::Service;

pub mod settings;

use settings::*;

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tokio::main]
async fn main() {
    // Initialize CLI app and get matches
    let matches = app_init_and_matches();

    // Collect settings
    let settings = Settings::new(matches).expect("failed to collect settings");

    // Collect metadata
    let bind_addr: SocketAddr = settings.bind.parse().expect("failed to parse bind address");
    let start_time = std::time::SystemTime::now();
    let metadata = Arc::new(services::Metadata {
        addr: bind_addr.clone(),
        start_time,
    });

    // Construct arena
    let arena = services::arena::Arena::default();

    // Construct player
    let database = services::database::Database::default();
    let player = services::player::Player::new(arena, metadata, database);

    // Create miners
    let site = services::miner::Site::default();
    let mut miner = services::miner::MiningCoordinator::new(3);
    miner.call(services::miner::NewSession(site)).await;

    // Create RPC
    let rpc_addr = settings
        .rpc_bind
        .parse()
        .expect("failed to parse rpc bind address");
    let rpc_server = rpc::RPCBuilder::default()
        .peering_service(player.clone())
        .info_service(
            get_version(),
            consensus::get_version(),
            network::get_version(),
            rpc::get_version(),
            miner::get_version(),
            crypto::get_version(),
        )
        .start(rpc_addr);

    let peer_acceptor = player.begin_acceptor();
    tokio::spawn(rpc_server);
    peer_acceptor.await;
}
