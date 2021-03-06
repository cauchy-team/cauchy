use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg, ArgMatches};
use config::{Config, ConfigError, File};
use serde::Deserialize;

const FOLDER_DIR: &str = ".cauchy";

pub fn app_init_and_matches<'a>() -> ArgMatches<'a> {
    App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .help("Sets a custom config file")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("bind")
                .short("b")
                .long("bind")
                .help("Sets the bind address")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("rpc-bind")
                .long("rpc-bind")
                .help("Sets the RPC bind address")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("mining-threads")
                .long("mining-threads")
                .help("Number of mining threads")
                .takes_value(true),
        )
        .get_matches()
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub bind: String,
    pub rpc_bind: String,
    pub radius: usize,
    pub mining_threads: u16,
}

impl Settings {
    pub fn new(matches: ArgMatches) -> Result<Self, ConfigError> {
        let mut s = Config::new();

        // Try get home directory
        let home_dir = match dirs::home_dir() {
            Some(some) => some,
            None => return Err(ConfigError::Message("no home directory".to_string())),
        };

        // Set default settings
        s.set_default("bind", "127.0.0.1:1080")?;
        s.set_default("rpc_bind", "0.0.0.0:2080")?;
        s.set_default("radius", 128)?;
        s.set_default("mining_threads", 1)?;

        // Load config from file
        let mut default_config = home_dir;
        default_config.push(format!("{}/config", FOLDER_DIR));
        let default_config_str = default_config.to_str().unwrap();
        let config_path = matches.value_of("config").unwrap_or(default_config_str);
        s.merge(File::with_name(config_path).required(false))?;

        // Try gather settings from cmd line
        if let Some(bind) = matches.value_of("bind") {
            s.set("bind", bind)?;
        }
        if let Some(rpc_bind) = matches.value_of("rpc-bind") {
            s.set("rpc_bind", rpc_bind)?;
        }
        if let Some(mining_threads) = matches.value_of("mining-threads") {
            s.set("mining_threads", mining_threads)?;
        }
        s.try_into()
    }
}
