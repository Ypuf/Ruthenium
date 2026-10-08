use crate::{ClientPacket, ServerPacket};

pub struct SStatusRequest;

impl<'a> ServerPacket<'a> for SStatusRequest {
    fn read(_read: &mut &'a [u8]) -> Result<Self, crate::ser::ReadError> {
        Ok(Self)
    }
}

impl ClientPacket for SStatusRequest {
    fn write_packet_data(
        &self,
        _write: impl std::io::prelude::Write,
    ) -> Result<(), crate::ser::WriteError> {
        Ok(())
    }
}
