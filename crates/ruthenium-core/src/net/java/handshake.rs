use std::sync::Arc;

use ruthenium_protocol::{ConnectionState, java::server::handshake::SHandShake};
use ruthenium_utils::version::JavaMinecraftVersion;
use tracing::debug;

use crate::{net::java::pending::PendingConnection, server::Server};

impl PendingConnection {
    pub async fn handle_handshake(&mut self, server: &Arc<Server>, handshake: SHandShake) {
        let version = handshake.protocol_version.0 as u32;
        self.server_address = handshake.server_address.to_string();
        self.version
            .store(JavaMinecraftVersion::from_protocol(version));

        debug!("Handshake: next state is {:?}", &handshake.next_state);
        self.connection_state.store(handshake.next_state);

        if self.connection_state.load() != ConnectionState::Status {
            let protocol = version;
            if protocol < JavaMinecraftVersion::V_26_3 as u32 {
                // kick -> outdated client
            } else if protocol > JavaMinecraftVersion::V_26_3 as u32 {
                // kick -> outdated server
            }
        }
    }
}
