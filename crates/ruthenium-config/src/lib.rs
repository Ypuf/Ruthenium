use std::{fs, path::Path};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::{debug, error, warn};

use crate::{commands::CommandsConfig, logging::LoggingConfig, networking::NetworkingConfig};

pub mod commands;
pub mod logging;
pub mod networking;

#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct RutheniumConfig {
    #[serde(flatten)]
    pub basic: BasicConfig,
    #[serde(flatten)]
    pub advanced: AdvancedConfig,
}

impl LoadConfig for RutheniumConfig {
    fn get_path() -> &'static Path {
        Path::new("ruthenium.toml")
    }
}

#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct AdvancedConfig {
    pub logging: LoggingConfig,
    pub networking: NetworkingConfig,
    pub commands: CommandsConfig,
}

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct BasicConfig {
    pub allow_nether: bool,
    pub allow_end: bool,
}

impl Default for BasicConfig {
    fn default() -> Self {
        Self {
            allow_nether: true,
            allow_end: true,
        }
    }
}

pub trait LoadConfig {
    fn load(conf_dir: &Path) -> Self
    where
        Self: Sized + Default + Serialize + DeserializeOwned,
    {
        if !conf_dir.exists() {
            debug!("creating config root dir");
            let _ = fs::create_dir(conf_dir);
        }

        let path = conf_dir.join(Self::get_path());

        let config = if path.exists() {
            let file_content = match fs::read_to_string(&path) {
                Ok(content) => content,
                Err(e) => {
                    error!("Failed to read config file at {}: {e}", path.display());
                    return Self::default();
                }
            };

            let parsed_toml_values: toml::Value = match toml::from_str(&file_content) {
                Ok(val) => val,
                Err(e) => {
                    error!("Failed to parse TOML at: {}. {e}", path.display());
                    return Self::default();
                }
            };

            let (merged_config, changed) = Self::merge_with_defaults(parsed_toml_values);

            if changed {
                let file_name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |f| f.to_string_lossy().into_owned(),
                );

                println!(
                    "{file_name} changed because values were missing. Defaults have been applied."
                );
                match toml::to_string(&merged_config) {
                    Ok(serialized) => {
                        if let Err(err) = fs::write(&path, serialized) {
                            warn!(
                                "Failed to write updated config to {}. {err}",
                                path.display()
                            );
                        }
                    }
                    Err(e) => {
                        warn!("Failed to serialize updated config {}. {e}", path.display());
                    }
                }
            }

            merged_config
        } else {
            let content = Self::default();
            match toml::to_string(&content) {
                Ok(serialized) => {
                    if let Err(e) = fs::write(&path, serialized) {
                        warn!("Failed to write default config to {}. {e}", path.display());
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to serialize default config for {:?}: {e}",
                        path.display()
                    );
                }
            }

            content
        };

        config
    }

    fn merge_with_defaults(parsed: toml::Value) -> (Self, bool)
    where
        Self: Sized + Default + Serialize + DeserializeOwned,
    {
        let default_config = Self::default();

        let Ok(default_toml_value) = toml::Value::try_from(&default_config) else {
            return (default_config, false);
        };

        let (merged_value, changed) = Self::merge_toml_values(default_toml_value, parsed);

        let config = merged_value.try_into().unwrap_or_else(|_| Self::default());

        (config, changed)
    }

    fn merge_toml_values(base: toml::Value, overlay: toml::Value) -> (toml::Value, bool) {
        match (base, overlay) {
            (toml::Value::Table(mut base_table), toml::Value::Table(overlay_table)) => {
                let mut changed = false;

                for key in base_table.keys() {
                    if !overlay_table.contains_key(key) {
                        changed = true;
                        break;
                    }
                }

                for (key, overlay_value) in overlay_table {
                    if let Some(base_value) = base_table.get(&key).cloned() {
                        let (merged_value, value_changed) =
                            Self::merge_toml_values(base_value, overlay_value);
                        base_table.insert(key, merged_value);
                        if value_changed {
                            changed = true;
                        }
                    } else {
                        base_table.insert(key, overlay_value);
                    }
                }
                (toml::Value::Table(base_table), changed)
            }
            (_, overlay) => (overlay, false),
        }
    }

    fn get_path() -> &'static Path;
}
