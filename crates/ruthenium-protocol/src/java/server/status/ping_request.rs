use crate::{
    ClientPacket, ServerPacket,
    ser::{NetworkReadExt, NetworkWriteExt},
};

pub struct SStatusPingRequest {
    pub payload: i64,
}

impl<'a> ServerPacket<'a> for SStatusPingRequest {
    fn read(read: &mut &'a [u8]) -> Result<Self, crate::ser::ReadError> {
        Ok(Self {
            payload: read.read_i64_be()?,
        })
    }
}

impl ClientPacket for SStatusPingRequest {
    fn write_packet_data(
        &self,
        mut write: impl std::io::prelude::Write,
    ) -> Result<(), crate::ser::WriteError> {
        write.write_i64_be(self.payload)?;
        Ok(())
    }
}
