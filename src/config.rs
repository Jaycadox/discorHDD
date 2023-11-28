use std::{
    io::{Read, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

/// The config, which should be stored in /etc/discorhdd/config.toml
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Config {
    /// A discord bot token
    pub token: String,
    /// A guild channel which the bot can read and write to
    pub channel: u64,
}

/// Gets/creates default config
pub fn get_config() -> Config {
    let config_path: PathBuf = "/etc/discorhdd/config.toml".into();
    let mut config = Config::default();
    match std::fs::File::open(&config_path) {
        Ok(mut file) => {
            let mut buf = String::new();
            file.read_to_string(&mut buf)
                .expect("Unable to read config file");
            config = toml::from_str(&buf).expect("invalid config file");
        }
        _ => {
            let out = toml::to_string(&config).expect("Unable to serialize default config");
            let _ = std::fs::create_dir(
                config_path
                    .parent()
                    .expect("Unable to find parent path of config"),
            );
            let mut file =
                std::fs::File::create(&config_path).expect("Unable to create config file");
            file.write_all(out.as_bytes())
                .expect("Unable to write to config file");
            println!(
                "Wrote default config to: {}",
                config_path.to_str().unwrap_or("invalid path")
            );
            println!("You probably want to configure the config before running again.");
        }
    }
    config
}
