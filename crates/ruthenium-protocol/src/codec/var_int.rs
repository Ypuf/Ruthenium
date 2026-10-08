use std::{
    io::{Error, ErrorKind, Read, Write},
    num::NonZero,
    ops::Deref,
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{
    ser::{NetworkReadExt, NetworkWriteExt, ReadError, WriteError},
    serial::{PacketRead, PacketWrite},
};

pub type VarIntType = i32;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VarInt(pub VarIntType);

impl VarInt {
    pub const MAX_SIZE: NonZero<usize> = NonZero::new(5).expect("5 is non zero");

    #[inline]
    pub const fn new(value: VarIntType) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn written_size(&self) -> usize {
        match self.0 as u32 {
            0 => 1,
            n => (31 - n.leading_zeros() as usize) / 7 + 1,
        }
    }

    #[inline]
    pub fn encode(&self, write: &mut impl Write) -> Result<(), WriteError> {
        let mut val = self.0 as u32;

        while val > 0x7F {
            write.write_u8((val as u8) | 0x80)?;
            val >>= 7;
        }

        write.write_u8(val as u8)?;
        Ok(())
    }

    #[inline]
    pub fn decode(read: &mut impl Read) -> Result<Self, ReadError> {
        let mut val = 0;
        for i in 0..Self::MAX_SIZE.get() {
            let byte = read.read_u8()?;

            val |= (i32::from(byte) & 0x7F) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(Self(val));
            }
        }
        Err(ReadError::TooLarge("VarInt".to_string()))
    }
}

impl VarInt {
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
            let byte = read.read_u8().await.map_err(|e| {
                if i == 0
                    && matches!(
                        e.kind(),
                        ErrorKind::UnexpectedEof
                            | ErrorKind::ConnectionRefused
                            | ErrorKind::ConnectionAborted
                            | ErrorKind::BrokenPipe
                    )
                {
                    ReadError::EOF("VarInt".to_string())
                } else {
                    ReadError::Incomplete(e.to_string())
                }
            })?;

            val |= (i32::from(byte) & 0x7F) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(Self(val));
            }
        }
        Err(ReadError::TooLarge("VarInt".to_string()))
    }
}

macro_rules! gen_from {
    ($type:ty) => {
        impl From<$type> for VarInt {
            fn from(value: $type) -> Self {
                VarInt(value.into())
            }
        }
    };
}

gen_from!(i8);
gen_from!(u8);
gen_from!(i16);
gen_from!(u16);
gen_from!(i32);

macro_rules! gen_try_from {
    ($type:ty) => {
        impl TryFrom<$type> for VarInt {
            type Error = <i32 as TryFrom<$type>>::Error;

            fn try_from(value: $type) -> Result<Self, Self::Error> {
                Ok(VarInt(value.try_into()?))
            }
        }
    };
}

gen_try_from!(u32);
gen_try_from!(i64);
gen_try_from!(u64);
gen_try_from!(isize);
gen_try_from!(usize);

impl AsRef<i32> for VarInt {
    fn as_ref(&self) -> &i32 {
        &self.0
    }
}

impl Deref for VarInt {
    type Target = i32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PacketWrite for VarInt {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        let mut val = ((self.0 << 1) ^ (self.0 >> 31)) as u32;

        while val > 0x7F {
            ((val as u8 & 0x7F) | 0x80).write(writer)?;
            val >>= 7;
        }

        (val as u8).write(writer)?;
        Ok(())
    }
}

impl PacketRead for VarInt {
    fn read<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let mut val = 0;
        for i in 0..Self::MAX_SIZE.get() {
            let byte = u8::read(reader)?;
            val |= u32::from(byte & 0x7F) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(Self(((val >> 1) as i32) ^ -((val & 1) as i32)));
            }
        }
        Err(Error::new(ErrorKind::InvalidData, "VarInt".to_string()))
    }
}
