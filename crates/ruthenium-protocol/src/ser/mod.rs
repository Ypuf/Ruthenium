use std::{
    fmt::Display,
    io::{Read, Write},
};

use thiserror::Error;

use crate::codec::{var_int::VarInt, var_uint::VarUInt};

#[derive(Debug, Error)]
pub enum ReadError {
    #[error("EOF, Tried to read {0} by No Bytes left.")]
    EOF(String),
    #[error("Incomplete: {0}")]
    Incomplete(String),
    #[error("Too Large: {0}")]
    TooLarge(String),
    #[error("{0}")]
    Message(String),
}

impl serde::de::Error for ReadError {
    fn custom<T: Display>(msg: T) -> Self {
        Self::Message(msg.to_string())
    }
}

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Serde Error: {0}")]
    Serde(String),
    #[error("{0}")]
    UnsupportedVersion(String),
    #[error("Failed to serialize packet: {0}")]
    Message(String),
}

impl serde::ser::Error for WriteError {
    fn custom<T: Display>(msg: T) -> Self {
        Self::Serde(msg.to_string())
    }
}

pub trait NetworkReadExt {
    fn read_i8(&mut self) -> Result<i8, ReadError>;
    fn read_u8(&mut self) -> Result<u8, ReadError>;

    fn read_i16_be(&mut self) -> Result<i16, ReadError>;
    fn read_u16_be(&mut self) -> Result<u16, ReadError>;
    fn read_i32_be(&mut self) -> Result<i32, ReadError>;
    fn read_u32_be(&mut self) -> Result<u32, ReadError>;
    fn read_i64_be(&mut self) -> Result<i64, ReadError>;
    fn read_u64_be(&mut self) -> Result<u64, ReadError>;
    fn read_f32_be(&mut self) -> Result<f32, ReadError>;
    fn read_f64_be(&mut self) -> Result<f64, ReadError>;
    fn read_i128_be(&mut self) -> Result<i128, ReadError>;
    fn read_u128_be(&mut self) -> Result<u128, ReadError>;

    #[inline]
    fn read_i16(&mut self) -> Result<i16, ReadError> {
        self.read_i16_be()
    }
    #[inline]
    fn read_u16(&mut self) -> Result<u16, ReadError> {
        self.read_u16_be()
    }
    #[inline]
    fn read_i32(&mut self) -> Result<i32, ReadError> {
        self.read_i32_be()
    }
    #[inline]
    fn read_u32(&mut self) -> Result<u32, ReadError> {
        self.read_u32_be()
    }
    #[inline]
    fn read_i64(&mut self) -> Result<i64, ReadError> {
        self.read_i64_be()
    }
    #[inline]
    fn read_u64(&mut self) -> Result<u64, ReadError> {
        self.read_u64_be()
    }
    #[inline]
    fn read_f32(&mut self) -> Result<f32, ReadError> {
        self.read_f32_be()
    }
    #[inline]
    fn read_f64(&mut self) -> Result<f64, ReadError> {
        self.read_f64_be()
    }

    fn read_bytes_to_buf(&mut self, buf: &mut [u8]) -> Result<(), ReadError>;

    fn read_bool(&mut self) -> Result<bool, ReadError>;
    fn read_var_int(&mut self) -> Result<VarInt, ReadError>;
    fn read_var_uint(&mut self) -> Result<VarUInt, ReadError>;
    // fn read_var_long(&mut self) -> Result<VarLong, ReadError>;
    // fn read_var_ulong(&mut self) -> Result<VarULong, ReadError>;
    fn read_str_bounded(&mut self, bound: usize) -> Result<Box<str>, ReadError>;
    #[inline]
    fn read_str(&mut self) -> Result<Box<str>, ReadError> {
        self.read_str_bounded(32767)
    }
    fn read_uuid(&mut self) -> Result<uuid::Uuid, ReadError>;

    #[inline]
    fn read_optional<G>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<G, ReadError>,
    ) -> Result<Option<G>, ReadError> {
        if self.read_bool()? {
            Ok(Some(parse(self)?))
        } else {
            Ok(None)
        }
    }
}

macro_rules! gen_read {
    ($name:ident, $type:ty) => {
        #[inline]
        fn $name(&mut self) -> Result<$type, ReadError> {
            let mut buf = [0u8; std::mem::size_of::<$type>()];
            self.read_exact(&mut buf)
                .map_err(|e| ReadError::Incomplete(e.to_string().into()))?;
            Ok(<$type>::from_be_bytes(buf))
        }
    };
}

impl<R: Read> NetworkReadExt for R {
    gen_read!(read_i8, i8);
    gen_read!(read_u8, u8);

    gen_read!(read_i16_be, i16);
    gen_read!(read_u16_be, u16);
    gen_read!(read_i32_be, i32);
    gen_read!(read_u32_be, u32);
    gen_read!(read_i64_be, i64);
    gen_read!(read_u64_be, u64);
    gen_read!(read_f32_be, f32);
    gen_read!(read_f64_be, f64);
    gen_read!(read_i128_be, i128);
    gen_read!(read_u128_be, u128);

    #[inline]
    fn read_bytes_to_buf(&mut self, buf: &mut [u8]) -> Result<(), ReadError> {
        self.read_exact(buf)
            .map_err(|e| ReadError::Incomplete(e.to_string()))
    }

    #[inline]
    fn read_bool(&mut self) -> Result<bool, ReadError> {
        let byte = self.read_u8()?;
        Ok(byte != 0)
    }

    #[inline]
    fn read_var_int(&mut self) -> Result<VarInt, ReadError> {
        VarInt::decode(self)
    }

    #[inline]
    fn read_var_uint(&mut self) -> Result<VarUInt, ReadError> {
        VarUInt::decode(self)
    }

    fn read_str_bounded(&mut self, bound: usize) -> Result<Box<str>, ReadError> {
        let bytes_len = self.read_var_uint()?.0 as usize;
        let maximum_utf8_bytes = bound.saturating_mul(3).min(crate::MAX_PACKET_DATA_SIZE);
        if bytes_len > maximum_utf8_bytes {
            return Err(ReadError::TooLarge(format!(
                "string has too many bytes ({bytes_len} > {maximum_utf8_bytes})"
            )));
        }

        if bytes_len <= 128 {
            let mut stack_buf = [0u8; 128];
            let slice = &mut stack_buf[..bytes_len];
            self.read_exact(slice)
                .map_err(|err| ReadError::Incomplete(err.to_string()))?;

            let string =
                std::str::from_utf8(slice).map_err(|e| ReadError::Message(e.to_string()))?;

            if string.encode_utf16().nth(bound).is_some() {
                return Err(ReadError::TooLarge(format!(
                    "string has too many UTF-16 characters (more than the maximum limit {bound})"
                )));
            }

            Ok(string.into())
        } else {
            let mut data = vec![0u8; bytes_len];
            self.read_bytes_to_buf(&mut data)?;
            let string =
                std::str::from_utf8(&data).map_err(|e| ReadError::Message(e.to_string()))?;

            if string.encode_utf16().nth(bound).is_some() {
                return Err(ReadError::TooLarge(format!(
                    "string has too many UTF-16 characters (more than the maximum limit {bound})"
                )));
            }

            Ok(string.into())
        }
    }

    fn read_uuid(&mut self) -> Result<uuid::Uuid, ReadError> {
        let mut bytes = [0u8; 16];
        self.read_exact(&mut bytes)
            .map_err(|e| ReadError::Incomplete(e.to_string()))?;
        Ok(uuid::Uuid::from_bytes(bytes))
    }
}

pub trait NetworkWriteExt {
    fn write_i8(&mut self, data: i8) -> Result<(), WriteError>;
    fn write_u8(&mut self, data: u8) -> Result<(), WriteError>;

    fn write_i16_be(&mut self, data: i16) -> Result<(), WriteError>;
    fn write_u16_be(&mut self, data: u16) -> Result<(), WriteError>;
    fn write_i32_be(&mut self, data: i32) -> Result<(), WriteError>;
    fn write_u32_be(&mut self, data: u32) -> Result<(), WriteError>;
    fn write_i64_be(&mut self, data: i64) -> Result<(), WriteError>;
    fn write_u64_be(&mut self, data: u64) -> Result<(), WriteError>;
    fn write_f32_be(&mut self, data: f32) -> Result<(), WriteError>;
    fn write_f64_be(&mut self, data: f64) -> Result<(), WriteError>;
    fn write_slice(&mut self, data: &[u8]) -> Result<(), WriteError>;

    fn write_i16(&mut self, data: i16) -> Result<(), WriteError> {
        self.write_i16_be(data)
    }
    fn write_u16(&mut self, data: u16) -> Result<(), WriteError> {
        self.write_u16_be(data)
    }
    fn write_i32(&mut self, data: i32) -> Result<(), WriteError> {
        self.write_i32_be(data)
    }
    fn write_u32(&mut self, data: u32) -> Result<(), WriteError> {
        self.write_u32_be(data)
    }
    fn write_i64(&mut self, data: i64) -> Result<(), WriteError> {
        self.write_i64_be(data)
    }
    fn write_u64(&mut self, data: u64) -> Result<(), WriteError> {
        self.write_u64_be(data)
    }
    fn write_f32(&mut self, data: f32) -> Result<(), WriteError> {
        self.write_f32_be(data)
    }
    fn write_f64(&mut self, data: f64) -> Result<(), WriteError> {
        self.write_f64_be(data)
    }

    fn write_bool(&mut self, data: bool) -> Result<(), WriteError> {
        if data {
            self.write_u8(1)
        } else {
            self.write_u8(0)
        }
    }
    fn write_var_int(&mut self, data: &VarInt) -> Result<(), WriteError>;
    fn write_var_uint(&mut self, data: &VarUInt) -> Result<(), WriteError>;
    fn write_string_bounded(&mut self, data: &str, bound: usize) -> Result<(), WriteError>;
    fn write_string(&mut self, data: &str) -> Result<(), WriteError>;

    fn write_uuid(&mut self, data: &uuid::Uuid) -> Result<(), WriteError> {
        let (first, second) = data.as_u64_pair();
        self.write_u64_be(first)?;
        self.write_u64_be(second)
    }

    fn write_optional<G>(
        &mut self,
        data: &Option<G>,
        writer: impl FnOnce(&mut Self, &G) -> Result<(), WriteError>,
    ) -> Result<(), WriteError> {
        if let Some(data) = data {
            self.write_bool(true)?;
            writer(self, data)
        } else {
            self.write_bool(false)
        }
    }
}

macro_rules! gen_write {
    ($name:ident, $type:ty) => {
        fn $name(&mut self, data: $type) -> Result<(), WriteError> {
            self.write_all(&data.to_be_bytes())
                .map_err(WriteError::IoError)
        }
    };
}

impl<W: Write> NetworkWriteExt for W {
    gen_write!(write_i8, i8);
    gen_write!(write_u8, u8);
    gen_write!(write_i16_be, i16);
    gen_write!(write_u16_be, u16);
    gen_write!(write_i32_be, i32);
    gen_write!(write_u32_be, u32);
    gen_write!(write_i64_be, i64);
    gen_write!(write_u64_be, u64);
    gen_write!(write_f32_be, f32);
    gen_write!(write_f64_be, f64);

    fn write_slice(&mut self, data: &[u8]) -> Result<(), WriteError> {
        self.write_all(data).map_err(WriteError::IoError)
    }

    fn write_var_int(&mut self, data: &VarInt) -> Result<(), WriteError> {
        data.encode(self)
    }

    fn write_var_uint(&mut self, data: &VarUInt) -> Result<(), WriteError> {
        data.encode(self)
    }

    fn write_string_bounded(&mut self, data: &str, bound: usize) -> Result<(), WriteError> {
        if data.len() > bound {
            return Err(WriteError::Message(format!(
                "string length {} exceeds bound {}",
                data.len(),
                bound
            )));
        }
        self.write_var_int(&data.len().try_into().map_err(|_| {
            WriteError::Message(format!("{} isnt representable as a VarInt", data.len()))
        })?)?;

        self.write_all(data.as_bytes()).map_err(WriteError::IoError)
    }

    fn write_string(&mut self, data: &str) -> Result<(), WriteError> {
        self.write_string_bounded(data, i16::MAX as usize)
    }
}
