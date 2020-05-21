pub mod settings;

use std::net::SocketAddr;

use settings::*;
use vm::{DefaultVM, VMFactory};

/// Get crate version.
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tokio::main]
async fn main() {
    // Init logging
    let env_filter = tracing_subscriber::EnvFilter::from_default_env();
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(env_filter)
        .init();

    // Initialize CLI app and get matches
    let matches = app_init_and_matches();

    // Collect settings
    let settings = Settings::new(matches).expect("failed to collect settings");

    // Create miners
    let miner = miner::MiningCoordinator::new(1);

    // Construct arena
    let arena: arena::Arena = arena::Arena::default();

    // Construct player
    let database = database::Database::default();
    let bind_addr: SocketAddr = settings.bind.parse().expect("failed to parse bind address");
    let player = player::Player::<_, VMFactory<DefaultVM>>::new(
        bind_addr,
        arena,
        miner.clone(),
        database,
        settings.radius,
    )
    .await;

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
        .mining_service(miner)
        .transactions_service(player.clone())
        .start(rpc_addr);

    // Peer acceptor task
    let peer_acceptor = player.clone().begin_acceptor();
    tokio::spawn(rpc_server);

    // Peer polling task
    let peer_poll = player.begin_heartbeat(3, 10_000);
    tokio::spawn(peer_poll);

    peer_acceptor.await;
}
