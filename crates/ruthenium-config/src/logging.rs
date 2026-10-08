use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub enabled: bool,
    pub level: String,
    pub threads: bool,
    pub thread_id: bool,
    pub target: bool,
    pub color: bool,
    pub timestamp: bool,
    pub timestamp_format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: "info".to_string(),
            threads: false,
            thread_id: false,
            target: false,
            color: true,
            timestamp: true,
            timestamp_format: "[hour]:[minute]:[second]".to_string(),
        }
    }
}
