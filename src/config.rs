// Main proxy configuration loaded from TOML.

use serde::{Deserialize, Deserializer};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Address to listen on (e.g. "0.0.0.0:10000")
    pub listen: String,

    /// Upstream Yellowstone gRPC address (e.g. "127.0.0.1:11000")
    pub upstream: String,

    /// Directory with per-IP rule files
    pub rules_dir: PathBuf,

    /// How often to reload rules from disk
    #[serde(
        default = "default_reload_interval",
        deserialize_with = "deserialize_duration"
    )]
    pub reload_interval: Duration,

    /// Log level (e.g. "info", "warn", "debug")
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Upstream idle timeout
    #[serde(default, deserialize_with = "deserialize_opt_duration")]
    pub upstream_idle_timeout: Option<Duration>,

    /// Upstream connection timeout
    #[serde(default, deserialize_with = "deserialize_opt_duration")]
    pub upstream_connection_timeout: Option<Duration>,

    /// TCP keepalive idle time
    #[serde(default, deserialize_with = "deserialize_opt_duration")]
    pub tcp_keepalive_idle: Option<Duration>,

    /// TCP keepalive probe interval
    #[serde(default, deserialize_with = "deserialize_opt_duration")]
    pub tcp_keepalive_interval: Option<Duration>,

    /// TCP keepalive probe count
    #[serde(default)]
    pub tcp_keepalive_count: Option<usize>,
}

fn default_reload_interval() -> Duration {
    Duration::from_secs(30)
}

fn default_log_level() -> String {
    "info".to_string()
}

fn deserialize_duration<'de, D>(d: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    humantime::parse_duration(&s).map_err(serde::de::Error::custom)
}

fn deserialize_opt_duration<'de, D>(d: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(d)?;
    match opt {
        Some(s) => humantime::parse_duration(&s)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

impl Config {
    pub fn load(path: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read config {path}: {e}"))?;
        toml::from_str(&content).map_err(|e| format!("failed to parse config: {e}"))
    }
}
