use std::{
    io::{Error, ErrorKind, Read, Write},
    num::NonZero,
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{
    ser::{NetworkReadExt, NetworkWriteExt, ReadError, WriteError},
    serial::{PacketRead, PacketWrite},
};

pub type VarUIntType = u32;

pub struct VarUInt(pub VarUIntType);

impl VarUInt {
    pub const MAX_SIZE: NonZero<usize> = NonZero::new(5).expect("5 is non zero");

    #[inline]
    pub const fn new(value: VarUIntType) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn written_size(&self) -> usize {
        match self.0 {
            0 => 1,
            n => (31 - n.leading_zeros() as usize) / 7 + 1,
        }
    }

    #[inline]
    pub fn encode(&self, write: &mut impl Write) -> Result<(), WriteError> {
        let mut val = self.0;
        loop {
            let mut byte = (val & 0x7F) as u8;
            val >>= 7;
            if val != 0 {
                byte |= 0x80;
            }
            write.write_u8(byte)?;
            if val == 0 {
                break;
            }
        }
        Ok(())
    }

    #[inline]
    pub fn decode(read: &mut impl Read) -> Result<Self, ReadError> {
        let mut val = 0;
        for i in 0..Self::MAX_SIZE.get() {
            let byte = read.read_u8()?;
            // Reject encodings that set payload bits beyond bit 31 in the final byte.
            if i == Self::MAX_SIZE.get() - 1 && byte & 0x70 != 0 {
                return Err(ReadError::TooLarge("VarUInt".to_string()));
            }
            val |= (u32::from(byte) & 0x7F) << (i * 7);
            if byte & 0x80 == 0 {
                return Ok(Self(val));
            }
        }
        Err(ReadError::TooLarge("VarUInt".to_string()))
    }
}

impl VarUInt {
    pub async fn encode_async(
        &self,
        write: &mut (impl AsyncWrite + Unpin),
    ) -> Result<(), WriteError> {
        let mut val = self.0;
        for _ in 0..Self::MAX_SIZE.get() {
            let b: u8 = val as u8 & 0b0111_1111;
            val >>= 7;
            write
                .write_u8(if val == 0 { b } else { b | 0b1000_0000 })
                .await
                .map_err(WriteError::IoError)?;
            if val == 0 {
                break;
            }
        }
        Ok(())
    }

    pub async fn decode_async(read: &mut (impl AsyncRead + Unpin)) -> Result<Self, ReadError> {
        let mut val = 0;
        for i in 0..Self::MAX_SIZE.get() {
            let byte = read.read_u8().await.map_err(|err| {
                if i == 0
                    && matches!(
                        err.kind(),
                        ErrorKind::UnexpectedEof
                            | ErrorKind::ConnectionReset
                            | ErrorKind::ConnectionAborted
                            | ErrorKind::BrokenPipe
                    )
                {
                    ReadError::EOF("VarUInt".to_string())
                } else {
                    ReadError::Incomplete(err.to_string())
                }
            })?;
            // Reject encodings that set payload bits beyond bit 31 in the final byte.
            if i == Self::MAX_SIZE.get() - 1 && byte & 0x70 != 0 {
                return Err(ReadError::TooLarge("VarUInt".to_string()));
            }
            val |= (u32::from(byte) & 0x7F) << (i * 7);
            if byte & 0x80 == 0 {
                return Ok(Self(val));
            }
        }
        Err(ReadError::TooLarge("VarUInt".to_string()))
    }
}

impl PacketWrite for VarUInt {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        let mut val = self.0;
        loop {
            let mut byte = (val & 0x7F) as u8;
            val >>= 7;
            if val != 0 {
                byte |= 0x80;
            }
            byte.write(writer)?;
            if val == 0 {
                break;
            }
        }
        Ok(())
    }
}

impl PacketRead for VarUInt {
    fn read<W: Read>(reader: &mut W) -> Result<Self, Error> {
        let mut val = 0;
        for i in 0..Self::MAX_SIZE.get() {
            let byte = u8::read(reader)?;

            val |= (u32::from(byte) & 0x7F) << (i * 7);
            if byte & 0x80 == 0 {
                return Ok(Self(val));
            }
        }
        Err(Error::new(ErrorKind::InvalidData, ""))
    }
}
