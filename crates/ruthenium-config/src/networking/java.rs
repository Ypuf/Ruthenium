use std::{net::SocketAddr, num::NonZero};

use serde::{Deserialize, Serialize};

use crate::networking::compression::CompressionConfig;

#[derive(Deserialize, Serialize, Clone)]
pub struct JavaConfig {
    pub enabled: bool,
    pub address: SocketAddr,
    pub encryption: bool,
    pub online_mode: bool,
    pub max_players: u32,
    pub view_distance: NonZero<u8>,
    pub simulation_distance: NonZero<u8>,
    #[serde(alias = "keep-alive-time", alias = "keep-alive-interval")]
    pub keep_alive_time: u64,
    pub compression: CompressionConfig,
    pub motd: String,
}

impl Default for JavaConfig {
    fn default() -> Self {
        let address = "0.0.0.0:25565"
            .parse()
            .unwrap_or_else(|_| std::net::SocketAddr::from(([0, 0, 0, 0], 25565)));
        let view_distance = NonZero::new(16).unwrap_or(NonZero::<u8>::MIN);
        let simulation_distance = NonZero::new(10).unwrap_or(NonZero::<u8>::MIN);

        Self {
            enabled: true,
            address,
            encryption: true,
            online_mode: true,
            max_players: 100,
            view_distance,
            simulation_distance,
            keep_alive_time: 15,
            compression: CompressionConfig::default(),
            motd: "meow".to_string(),
        }
    }
}
