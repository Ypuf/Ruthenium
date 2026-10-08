use crate::{
    ClientPacket, ConnectionState, ServerPacket,
    codec::var_int::VarInt,
    ser::{NetworkReadExt, NetworkWriteExt, ReadError, WriteError},
};

pub struct SHandShake {
    pub protocol_version: VarInt,
    pub server_address: Box<str>,
    pub server_port: u16,
    pub next_state: ConnectionState,
}

impl<'a> ServerPacket<'a> for SHandShake {
    fn read(read: &mut &'a [u8]) -> Result<Self, ReadError> {
        Ok(Self {
            protocol_version: read.read_var_int()?,
            server_address: read.read_str_bounded(i16::MAX as usize)?,
            server_port: read.read_u16_be()?,
            next_state: read
                .read_var_int()?
                .try_into()
                .map_err(|_| ReadError::Message("Invalid status".to_string()))?,
        })
    }
}

impl ClientPacket for SHandShake {
    fn write_packet_data(&self, mut write: impl std::io::Write) -> Result<(), WriteError> {
        write.write_var_int(&self.protocol_version)?;
        write.write_string(&self.server_address)?;
        write.write_u16_be(self.server_port)?;
        write.write_var_int(&VarInt(self.next_state as i32))?;
        Ok(())
    }
}
