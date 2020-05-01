pub mod settings;

use settings::*;

#[tokio::main]
async fn main() {
    // Initialize CLI app and get matches
    let matches = app_init_and_matches();

    // Collect settings
    let settings = Settings::new(matches).expect("failed to collect settings");

    
}
