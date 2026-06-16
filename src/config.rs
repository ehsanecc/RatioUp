use std::path::PathBuf;
use std::str::FromStr;

use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct Config {
    /// torrent port
    pub port: u16,
    pub min_upload_rate: u32,
    pub max_upload_rate: u32,

    pub use_pid_file: bool,

    /// To set the number of peers we want
    pub numwant: Option<u16>,
    pub client: String,
    /// Directory where torrents are saved. Default is in the working directory.
    pub torrent_dir: PathBuf,
    /// Output file path for the JSON file.
    /// You may want something like `/var/www/ratio_up.json` to expose it on your web server.
    pub output_stats: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            port: fastrand::u16(49152..65534),
            min_upload_rate: 8192,
            max_upload_rate: 2097152,
            use_pid_file: false,
            numwant: None,
            torrent_dir: PathBuf::from("."),
            client: String::from("Transmission_3_00"),
            output_stats: None,
        }
    }
}

impl Config {
    pub async fn load_from_file(path: &PathBuf) -> Config {
        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                let config_value = match toml_span::parse(&content) {
                    Ok(val) => val,
                    Err(e) => {
                        error!("Cannot load config file: {e}");
                        return Config::default();
                    }
                };

                let mut config = Config::default();

                let root_table = match config_value.as_table() {
                    Some(table) => table,
                    None => {
                        error!("Invalid type in config file");
                        return config;
                    }
                };

                if let Some(v) = root_table.get("client") {
                    if let Some(s) = v.as_str() {
                        config.client = String::from(s);
                    } else {
                        error!("client is not a string");
                    }
                }

                if let Some(v) = root_table.get("port") {
                    if let Some(port) = v.as_integer() {
                        if !(1..=65535).contains(&port) {
                            error!("Invalid port");
                        } else {
                            config.port = port as u16;
                        }
                    } else {
                        error!("port is not an integer");
                    }
                }

                if let Some(v) = root_table.get("numwant") {
                    if let Some(numwant) = v.as_integer() {
                        if !(1..=65535).contains(&numwant) {
                            error!("Invalid numwant");
                        } else {
                            config.numwant = Some(numwant as u16);
                        }
                    } else {
                        error!("numwant is not an integer");
                    }
                }

                if let Some(v) = root_table.get("use_pid_file") {
                    if let Some(b) = v.as_bool() {
                        config.use_pid_file = b;
                    } else {
                        error!("use_pid_file is not a boolean");
                    }
                }

                if let Some(v) = root_table.get("min_upload_rate") {
                    if let Some(value) = v.as_integer() {
                        config.min_upload_rate = value as u32;
                    } else {
                        error!("Invalid min_upload_rate");
                        return config;
                    }
                }

                if let Some(v) = root_table.get("max_upload_rate") {
                    if let Some(value) = v.as_integer() {
                        config.max_upload_rate = value as u32;
                    } else {
                        error!("Invalid max_upload_rate");
                        return config;
                    }
                }

                if let Some(v) = root_table.get("torrent_dir") {
                    if let Some(s) = v.as_str() {
                        config.torrent_dir = PathBuf::from(s);
                    } else {
                        error!("Invalid torrent_dir");
                    }
                }

                if let Some(v) = root_table.get("output_stats") {
                    if let Some(s) = v.as_str() {
                        config.output_stats = Some(PathBuf::from(s));
                    } else {
                        error!("Invalid output_stats");
                    }
                }

                if !config.speeds_ok() {
                    warn!(
                        "Min upload rate ({}) is greater than max upload rate ({}), switching values",
                        config.min_upload_rate, config.max_upload_rate
                    );
                    std::mem::swap(&mut config.min_upload_rate, &mut config.max_upload_rate);
                }

                config
            }
            Err(e) => {
                error!("Could not read config file: {} {e}", path.display());
                info!("Using default configuration");
                Config::default()
            }
        }
    }

    fn speeds_ok(&self) -> bool {
        self.min_upload_rate <= self.max_upload_rate
    }
}

/// Init the client from the configuration and returns the interval to refresh client key if applicable
pub async fn init_client(config: &Config) -> Option<u16> {
    let mut client = fake_torrent_client::Client::default();
    match fake_torrent_client::clients::ClientVersion::from_str(&config.client) {
        Ok(selected) => {
            client.build(selected);
        }
        Err(e) => {
            error!(
                "Client {} does not exist, using default one: {e}",
                config.client
            );
        }
    }
    info!(
        "Client {} (key: {}, peer ID:{})",
        client.name, client.key, client.peer_id
    );
    let key_interval = client.key_refresh_every;
    let user_agent = client.user_agent.clone();
    let mut guard = crate::CLIENT.write().await;
    *guard = Some(client);
    drop(guard);

    let reqwest_client = reqwest::Client::builder()
        .user_agent(user_agent)
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("Failed to build HTTP client");
    let _ = crate::HTTP_CLIENT.set(reqwest_client);

    key_interval
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

    #[test]
    fn test_speed_ok() {
        let mut cfg = Config::default();
        assert!(cfg.speeds_ok());

        cfg.min_upload_rate = 8192;
        cfg.max_upload_rate = 8192;
        assert!(cfg.speeds_ok());

        cfg.min_upload_rate = 8192;
        cfg.max_upload_rate = 4096;
        assert!(!cfg.speeds_ok());
    }
}
