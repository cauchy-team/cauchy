use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg, ArgMatches};
use config::{Config, ConfigError, File};
use serde::Deserialize;

const FOLDER_DIR: &str = ".keyserver";

pub fn app_init_and_matches<'a>() -> ArgMatches<'a> {
    App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .arg(
            Arg::with_name("bind")
                .short("b")
                .long("bind")
                .help("Sets the bind address")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .help("Sets a custom config file")
                .takes_value(true),
        )
        .get_matches()
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub bind: String,
}

impl Settings {
    pub fn new<'a>(matches: ArgMatches<'a>) -> Result<Self, ConfigError> {
        let mut s = Config::new();

        // Try get home directory
        let home_dir = match dirs::home_dir() {
            Some(some) => some,
            None => return Err(ConfigError::Message("no home directory".to_string())),
        };

        // Set default settings
        s.set_default("bind", "127.0.0.1:2332")?;

        // Load config from file
        let mut default_config = home_dir.clone();
        default_config.push(format!("{}/config", FOLDER_DIR));
        let default_config_str = default_config.to_str().unwrap();
        let config_path = matches.value_of("config").unwrap_or(default_config_str);
        s.merge(File::with_name(config_path).required(false))?;

        // Try set bind address from cmd line
        if let Some(bind) = matches.value_of("bind") {
            s.set("bind", bind)?;
        }

        s.try_into()
    }
}
