pub mod settings;

use std::sync::Arc;

use futures::prelude::*;

use network::NetworkManager;
use settings::*;

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

    while let Some(new_conn) = connection_stream.next().await {
        println!("connected to {}", new_conn.addr);
    }
}
