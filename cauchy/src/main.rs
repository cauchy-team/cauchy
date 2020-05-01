use std::{net::SocketAddr, sync::Arc};

use futures::stream::StreamExt;
use tokio::net::TcpListener;
use tower::util::ServiceExt;

pub mod settings;

use settings::*;

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub async fn begin_acceptor(bind_addr: SocketAddr, player: services::player::Player) {
    // Listen for peers
    let mut listener = TcpListener::bind(bind_addr)
        .await
        .expect("failed to bind address");
    let filtered_listener = listener
        .incoming()
        .filter_map(|res| async move { res.ok() });
    let mut response_stream = Box::pin(player.clone().call_all(filtered_listener));

    while let Some(_) = response_stream.next().await {

    }
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

    let peer_acceptor = begin_acceptor(bind_addr, player);
    tokio::spawn(peer_acceptor);
    rpc_server.await;

}
