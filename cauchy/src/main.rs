pub mod settings;

use futures::{channel::mpsc, prelude::*};
use std::net::SocketAddr;

use network::NetworkHandles;
use rpc::RPCBuilder;
use settings::*;

fn wire_rpc(network_manager: &NetworkHandles, addr: String) {
    const NEW_PEER_CHANNEL_CAPACITY: usize = 256;

    // Setup connect peer RPC
    let (connect_sink, connect_stream) =
        mpsc::channel::<rpc::peering::ConnectMessage>(NEW_PEER_CHANNEL_CAPACITY);
    let new_connection_sink = network_manager.new_connection_sink();
    let new_peers = connect_stream.for_each(move |msg| {
        let mut new_connection_sink = new_connection_sink.clone();
        async move {
            let socket: SocketAddr = msg
                .address
                .parse()
                .expect("TODO return error through callback");
            let new_connection = network::NewConnection {
                socket,
                callback: msg.callback,
            };
            new_connection_sink
                .send(new_connection)
                .await
                .expect("tcp stream sender dropped");
        }
    });
    tokio::spawn(new_peers);

    // Build RPC
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
        .peering_service(connect_sink);
    let rpc_addr = addr.parse().expect("malformed rpc address");
    tokio::spawn(rpc.start(rpc_addr));
}

#[tokio::main]
async fn main() {
    // Initialize CLI app and get matches
    let matches = app_init_and_matches();

    // Gather settings
    let settings = Settings::new(matches).expect("failed to gather settings");

    // Construct arena
    // let arena = Arc::new(arena::Arena::default());

    // Initialize networking
    let mut network_manager = NetworkHandles::build()
        .bind(settings.bind)
        .start()
        .await
        .expect("could not start networking");

    // Take stream of new connections
    let mut connection_stream = network_manager.connection_stream().unwrap(); // This is safe

    // Initialize RPC
    wire_rpc(&network_manager, settings.rpc_bind);

    while let Some(new_conn) = connection_stream.next().await {
        println!("connected to {}", new_conn.addr);
    }
}
