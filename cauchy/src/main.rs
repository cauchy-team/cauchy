pub mod settings;

use std::sync::Arc;

use futures::{channel::mpsc, prelude::*};
use std::net::SocketAddr;
use tokio::net::TcpStream;

use network::NetworkManager;
use rpc::RPCBuilder;
use settings::*;

fn wire_rpc(network_manager: &NetworkManager, addr: String) {
    const NEW_PEER_CHANNEL_CAPACITY: usize = 256;

    let (new_peer_sender, new_peer_recv) = mpsc::channel(NEW_PEER_CHANNEL_CAPACITY);
    let rpc = RPCBuilder::default()
        .info_service(
            clap::crate_version!().to_string(),
            consensus::get_version(),
            network::get_version(),
            rpc::get_version(),
            arena::get_version(),
            miner::get_version(),
            crypto::get_version(),
        )
        .peering_service(new_peer_sender);
    let rpc_addr = addr.parse().expect("malformed rpc address");
    tokio::spawn(rpc.start(rpc_addr));
    let send_tcp_stream = network_manager.get_tcp_stream_sender();
    let new_peers = new_peer_recv.for_each(move |msg| {
        let mut send_tcp_stream_inner = send_tcp_stream.clone();
        async move {
            let peer_addr: SocketAddr = msg.address.parse().expect("malformed rpc address");
            match TcpStream::connect(peer_addr).await {
                Ok(tcp_stream) => {
                    send_tcp_stream_inner
                        .send(tcp_stream)
                        .await
                        .expect("tcp stream sender dropped");
                    msg.callback
                        .send(Ok(()))
                        .expect("tcp stream sender dropped");
                }
                Err(err) => {
                    msg.callback
                        .send(Err(err))
                        .expect("tcp stream sender dropped");
                }
            };
        }
    });
    tokio::spawn(new_peers);
}

#[tokio::main]
async fn main() {
    // Initialize CLI app and get matches
    let matches = app_init_and_matches();

    // Gather settings
    let settings = Settings::new(matches).expect("failed to gather settings");

    // Construct arena
    let arena = Arc::new(arena::Arena::default());

    // Initialize networking
    let mut network_manager = NetworkManager::build()
        .bind(settings.bind)
        .start()
        .await
        .expect("could not start network manager");

    // Take stream of new connections
    let mut connection_stream = network_manager.connection_stream().unwrap(); // This is safe

    // Initialize RPC
    wire_rpc(&network_manager, settings.rpc_bind);

    while let Some(new_conn) = connection_stream.next().await {
        println!("connected to {}", new_conn.addr);
    }
}
