use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct CommandsConfig {
    pub use_console: bool,
    pub use_tty: bool,
    pub log_console: bool,
}

impl Default for CommandsConfig {
    fn default() -> Self {
        Self {
            use_console: true,
            log_console: true,
            use_tty: true,
        }
    }
}
