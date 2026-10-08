use std::time::Instant;

use rsa::{RsaPrivateKey, pkcs8::EncodePublicKey};
use tracing::{debug, error};

pub struct KeyStore {
    pub private_key: RsaPrivateKey,
    pub public_key_der: Box<[u8]>,
}

impl KeyStore {
    pub fn new() -> Self {
        let instant = Instant::now();
        debug!("Generating key pair...");
        let private_key = Self::generate_private_key();

        let public_key = private_key.to_public_key();

        let public_key_der = public_key
            .to_public_key_der()
            .map(|der| der.into_vec().into_boxed_slice())
            .unwrap_or_default();

        debug!("Created RSA keys, took {}ms", instant.elapsed().as_millis());

        Self {
            private_key,
            public_key_der,
        }
    }

    fn generate_private_key() -> RsaPrivateKey {
        let mut rng = rand::rng();

        RsaPrivateKey::new(&mut rng, 1024).unwrap_or_else(|_| {
            error!("Failed to generate RSA key");
            std::process::exit(1);
        })
    }
}
