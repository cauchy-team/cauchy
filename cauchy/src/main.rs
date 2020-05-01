pub mod settings;

use settings::*;

#[tokio::main]
async fn main() {
    // Initialize CLI app and get matches
    let matches = app_init_and_matches();

    // Gather settings
    let settings = Settings::new(matches).expect("failed to gather settings");
}
