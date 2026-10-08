use serde::{Deserialize, Serialize};

use crate::networking::java::JavaConfig;

pub mod compression;
pub mod java;

#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct NetworkingConfig {
    pub java: JavaConfig,
}
