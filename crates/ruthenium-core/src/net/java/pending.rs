use std::{
    net::SocketAddr,
    sync::{Arc, Weak},
};

use bytes::Bytes;
use crossbeam::atomic::AtomicCell;
use ruthenium_config::networking::compression::CompressionInfo;
use ruthenium_protocol::{
    ConnectionState, PacketDecodeError, RawPacket, ServerPacket,
    java::{
        packet_decoder::TCPNetworkDecoder,
        server::{handshake::SHandShake, status::SStatusRequest},
    },
    ser::ReadError,
};
use ruthenium_utils::version::JavaMinecraftVersion;
use tokio::{
    io::{BufReader, BufWriter},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

use crate::{
    net::{PacketHandlerResult, PlayerConfig},
    server::Server,
};

const HANDSHAKE_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub struct PendingConnection {
    pub id: u64,
    pub address: SocketAddr,
    pub server_address: String,
    pub version: AtomicCell<JavaMinecraftVersion>,
    pub connection_state: AtomicCell<ConnectionState>,
    pub close_token: CancellationToken,
    // pub network_writer: TCPNetworkEncoder<BufWriter<OwnedWriteHalf>>,
    pub network_reader: TCPNetworkDecoder<BufReader<OwnedReadHalf>>,
    pub config: Option<PlayerConfig>,
    pub verify_token: Option<[u8; 4]>,
    server: Weak<Server>,
}

impl PendingConnection {
    pub fn new(tcp_stream: TcpStream, address: SocketAddr, id: u64, server: Weak<Server>) -> Self {
        let (read, _write) = tcp_stream.into_split();
        Self {
            id,
            address,
            server_address: String::new(),
            version: AtomicCell::new(JavaMinecraftVersion::V_26_3),
            connection_state: AtomicCell::new(ConnectionState::HandShake),
            close_token: CancellationToken::new(),
            network_reader: TCPNetworkDecoder::new(BufReader::new(read)),
            config: None,
            verify_token: None,
            server,
        }
    }

    pub fn close(&self) {
        self.close_token.cancel();
    }

    pub fn is_closed(&self) -> bool {
        self.close_token.is_cancelled()
    }

    pub async fn await_close_interrupt(&self) {
        self.close_token.cancelled().await
    }

    pub fn set_encryption(
        &mut self,
        _shared_secret: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Change error type.
        todo!();
    }

    pub fn set_compression(&mut self, compression: &CompressionInfo) {
        if compression.level > 9 {
            error!("Invalid compression level.");
        }

        self.network_reader
            .set_compression(compression.threshold as usize);
        // self.network_writer
        //     .set_compression((compression.threshold as usize, compression.level));
    }

    pub async fn get_packet(&mut self) -> Option<RawPacket> {
        let close_token = self.close_token.clone();
        let packet_result = tokio::select! {
            () = close_token.cancelled() => {
                debug!("pending connection closed during packet processing");
                return None;
            },
            () = tokio::time::sleep(HANDSHAKE_IDLE_TIMEOUT) => {
                debug!("Client {} sent nothing for {}s. Closing connection.", self.id, HANDSHAKE_IDLE_TIMEOUT.as_secs());
                return None;
            },
            res = self.network_reader.get_raw_packet() => res,
        };

        match packet_result {
            Ok(packet) => Some(packet),
            Err(e) => {
                if !matches!(e, PacketDecodeError::ConnectionClosed) {
                    debug!("Failed to decode packet from client {}: {}", self.id, e);
                    // let text = format!("Error reading incoming packet {e}");
                    // self.kick().await;
                    ()
                }
                None
            }
        }
    }

    pub async fn kick(&mut self, _reason: &str) {
        todo!()
    }

    pub async fn handle_login_sequence(&mut self, server: &Arc<Server>) -> PacketHandlerResult {
        while let Some(packet) = self.get_packet().await {
            match self.handle_packet(server, &packet).await {
                Ok(result) => {
                    if let Some(result) = result {
                        return result;
                    }
                }
                Err(err) => {
                    let _text = format!("Error while reading incoming packet {err}");
                    debug!(
                        "Failed to read incoming packet with id {}: {}",
                        packet.id, err
                    );
                    // self.kick(text).await;
                }
            }
        }
        PacketHandlerResult::Stop
    }

    pub async fn handle_packet(
        &mut self,
        server: &Arc<Server>,
        packet: &RawPacket,
    ) -> Result<Option<PacketHandlerResult>, ReadError> {
        match self.connection_state.load() {
            ConnectionState::HandShake => self.handle_handshake_packet(server, packet).await,
            ConnectionState::Status => self.handle_status_packet(server, packet).await,
            ConnectionState::Login => self.handle_login_packet(server, packet).await,
            ConnectionState::Config => self.handle_config_packet(server, packet).await,
            ConnectionState::Play => Ok(None),
        }
    }

    async fn handle_handshake_packet(
        &mut self,
        server: &Arc<Server>,
        packet: &RawPacket,
    ) -> Result<Option<PacketHandlerResult>, ReadError> {
        debug!("Handling handshake state");
        let mut payload = &packet.payload[..];
        debug!("packet id: {}", packet.id);

        match packet.id {
            0 => {
                self.handle_handshake(server, SHandShake::read(&mut payload)?)
                    .await;
                Ok(None)
            }
            _ => Err(ReadError::Message(format!(
                "Failed to handle packet id {} in handshake state",
                packet.id
            ))),
        }
    }

    async fn handle_status_packet(
        &mut self,
        _server: &Arc<Server>,
        packet: &RawPacket,
    ) -> Result<Option<PacketHandlerResult>, ReadError> {
        debug!("Handling status state");
        let mut _payload = &packet.payload[..];
        match packet.id {
            _ => Err(ReadError::Message(format!(
                "Failed to handle packet id {} in status state",
                packet.id
            ))),
        }
    }

    async fn handle_login_packet(
        &mut self,
        _server: &Arc<Server>,
        packet: &RawPacket,
    ) -> Result<Option<PacketHandlerResult>, ReadError> {
        debug!("Handling login state");
        let mut _payload = &packet.payload[..];
        match packet.id {
            _ => Err(ReadError::Message(format!(
                "Failed to handle packet id {} in login state",
                packet.id
            ))),
        }
    }

    async fn handle_config_packet(
        &mut self,
        _server: &Arc<Server>,
        packet: &RawPacket,
    ) -> Result<Option<PacketHandlerResult>, ReadError> {
        debug!("Handling config state");
        let mut _payload = &packet.payload[..];
        match packet.id {
            _ => Err(ReadError::Message(format!(
                "Failed to handle packet id {} in config state",
                packet.id
            ))),
        }
    }
}
