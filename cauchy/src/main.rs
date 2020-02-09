pub mod settings;

use network::NetworkManager;
use settings::*;

use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Initialize CLI app and get matches
    let matches = app_init_and_matches();

    // Gather settings
    let settings = Settings::new(matches).expect("failed to read settings");

    // Construct arena
    let arena = Arc::new(arena::Arena::default());

    // Initialize networking
    let network_manager = NetworkManager::build().bind(settings.bind).start().await;
}
