use std::{num::NonZero, sync::Arc};

use arc_swap::ArcSwap;
use ruthenium_protocol::Property;
use serde::{Deserialize, Deserializer};
use thiserror::Error;
use uuid::Uuid;

pub mod java;

#[derive(Deserialize, Debug)]
pub struct GameProfile {
    pub id: Uuid,
    pub name: String,
    #[serde(deserialize_with = "from_vec")]
    pub properties: ArcSwap<Vec<Property>>,
    // #[serde(rename = "profileActions")]
    // pub profile_actions: Option<Vec<ProfileAction>>,
}

impl Clone for GameProfile {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            name: self.name.clone(),
            properties: ArcSwap::new(self.properties.load().clone()),
        }
    }
}

fn from_vec<'de, D>(deserializer: D) -> Result<ArcSwap<Vec<Property>>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Vec::<Property>::deserialize(deserializer)?;
    Ok(ArcSwap::new(Arc::new(v)))
}

#[derive(Clone)]
pub struct PlayerConfig {
    pub locale: String,
    pub view_distance: NonZero<u8>,
    // pub chat_mode: ChatMode,
    pub chat_colors: bool,
    pub skin_parts: u8,
    // pub main_hand: Hand,
    pub text_filtering: bool,
    pub server_listing: bool,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            locale: "en_us".to_string(),
            view_distance: NonZero::new(8).unwrap_or(NonZero::<u8>::MIN),
            chat_colors: true,
            skin_parts: 0x7F,
            text_filtering: false,
            server_listing: false,
        }
    }
}

pub enum PacketHandlerResult {
    Stop,
    ReadyToPlay(GameProfile, PlayerConfig),
}

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("failed to decrypt shared secret")]
    FailedDecrypt,
    #[error("shared secret has the wrong length")]
    SharedWrongLength,
    #[error("encryption is already enabled")]
    AlreadyEncrypted,
    #[error("no encryption request is pending")]
    NoPendingVerifyToken,
    #[error("verify token does not match")]
    VerifyTokenMismatch,
}
