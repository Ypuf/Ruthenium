use std::{
    io::{Error, Write},
    pin::Pin,
    task::{Context, Poll},
};

use aes::cipher::BlockSizeUser;
use bytes::Bytes;
use hybrid_array::{Array, sizes::U1};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::{
    codec::var_int::VarInt,
    ser::{ReadError, WriteError},
};

pub mod codec;
pub mod java;
pub mod packet;
pub mod ser;
pub mod serial;

pub const MAX_PACKET_SIZE: u64 = 2_097_151;
pub const MAX_PACKET_DATA_SIZE: usize = 8_388_608;

/// Represents a compression threshold.
///
/// Threshold determines the min size that should be compressed.
/// Data smaller than the threshold won't be compressed.
pub type CompressionThreshold = usize;

/// Represents a compression level.
///
/// Level determines the amount of compresion applied to the data.
/// Higher levels usually result in higher compression ratios
/// but also increase CPU usage.
pub type CompressionLevel = u32;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ConnectionState {
    HandShake,
    Status,
    Login,
    Config,
    Play,
}
pub struct InvalidConnectionState;

impl TryFrom<VarInt> for ConnectionState {
    type Error = InvalidConnectionState;

    fn try_from(value: VarInt) -> Result<Self, Self::Error> {
        let value = value.0;
        match value {
            1 => Ok(ConnectionState::Status),
            2 => Ok(ConnectionState::Login),
            _ => Err(InvalidConnectionState),
        }
    }
}

type Aes128Cfb8Dec = cfb8::Decryptor<aes::Aes128>;

pub struct StreamDecryptor<R: AsyncRead + Unpin> {
    cipher: Aes128Cfb8Dec,
    reader: R,
}

impl<R: AsyncRead + Unpin> StreamDecryptor<R> {
    pub const fn new(cipher: Aes128Cfb8Dec, stream: R) -> Self {
        Self {
            cipher,
            reader: stream,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for StreamDecryptor<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let ref_self = self.get_mut();
        let reader = Pin::new(&mut ref_self.reader);
        let cipher = &mut ref_self.cipher;

        // get original position
        let original_fill = buf.filled().len();
        // read raw data
        let internal_poll = reader.poll_read(cx, buf);

        if matches!(internal_poll, Poll::Ready(Ok(()))) {
            // Decrypt raw data in-place
            for block in buf.filled_mut()[original_fill..].chunks_mut(Aes128Cfb8Dec::block_size()) {
                cipher.decrypt(block);
            }
        }

        internal_poll
    }
}

type Aes128Cfb8Enc = cfb8::Encryptor<aes::Aes128>;

pub struct StreamEncryptor<W: AsyncWrite + Unpin> {
    cipher: Aes128Cfb8Enc,
    writer: W,
    last_unwritten_encrypted_byte: Option<u8>,
}

impl<W: AsyncWrite + Unpin> StreamEncryptor<W> {
    pub fn new(cipher: Aes128Cfb8Enc, stream: W) -> Self {
        debug_assert_eq!(Aes128Cfb8Enc::block_size(), 1);
        Self {
            cipher,
            writer: stream,
            last_unwritten_encrypted_byte: None,
        }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for StreamEncryptor<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        let ref_self = self.get_mut();
        let cipher = &mut ref_self.cipher;

        let mut total_written = 0;
        // Decrypt raw data
        for block in buf.chunks(Aes128Cfb8Enc::block_size()) {
            let mut out = [0u8];

            if let Some(last) = ref_self.last_unwritten_encrypted_byte {
                out[0] = last;
            } else {
                let out_block: &mut Array<u8, U1> = (&mut out[..])
                    .try_into()
                    .map_err(|_| Error::other("Output slice does not match with block size"))?;
                cipher
                    .encrypt_b2b(block, out_block)
                    .map_err(|_| Error::other("Encryption failed"))?;
            }

            let writer = Pin::new(&mut ref_self.writer);
            match writer.poll_write(cx, &out) {
                Poll::Pending => {
                    ref_self.last_unwritten_encrypted_byte = Some(out[0]);
                    if total_written == 0 {
                        // Didnt write anything... pending...
                        return Poll::Pending;
                    }
                    // Actually wrote something
                    return Poll::Ready(Ok(total_written));
                }
                Poll::Ready(result) => {
                    ref_self.last_unwritten_encrypted_byte = None;
                    match result {
                        Ok(written) => total_written += written,
                        Err(e) => return Poll::Ready(Err(e)),
                    }
                }
            }
        }

        Poll::Ready(Ok(total_written))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let ref_self = self.get_mut();
        let writer = Pin::new(&mut ref_self.writer);
        writer.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let ref_self = self.get_mut();
        let writer = Pin::new(&mut ref_self.writer);
        writer.poll_shutdown(cx)
    }
}

pub struct RawPacket {
    pub id: i32,
    pub payload: Bytes,
}

pub trait ClientPacket {
    fn write_packet_data(&self, write: impl Write) -> Result<(), WriteError>;

    fn write_packet(&self, _write: impl Write) -> Result<(), WriteError> {
        todo!("write_packet")
    }

    fn serialize_packet(&self) -> Result<Bytes, WriteError> {
        todo!("serialize_packet")
    }
}

pub trait ServerPacket<'a>: Sized {
    fn read(read: &mut &'a [u8]) -> Result<Self, ReadError>;
}

#[derive(Debug, Error)]
pub enum PacketEncodeError {
    #[error("Packet is too long: {0}")]
    TooLong(usize),
    #[error("Compression failed: {0}")]
    CompressionFailed(String),
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Error)]
pub enum PacketDecodeError {
    #[error("Failed to decode packet ID")]
    DecodeID,
    #[error("Packet is too long")]
    TooLong,
    #[error("Packet length is out of bounds")]
    OutOfBounds,
    #[error("Malformed length: {0}")]
    MalformedLength(String),
    #[error("Failed to decompress packet: {0}")]
    FailedDecompression(String),
    #[error("Packet is not compressed")]
    NotCompressed,
    #[error("Connection closed")]
    ConnectionClosed,
    #[error("{0}")]
    Message(String),
}

impl From<ReadError> for PacketDecodeError {
    fn from(value: ReadError) -> Self {
        Self::FailedDecompression(value.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Property {
    pub name: Box<str>,
    pub value: Box<str>,
    pub signature: Option<Box<str>>,
}

impl Property {
    pub fn read(&self, read: &mut impl ser::NetworkReadExt) -> Result<Self, ser::ReadError> {
        Ok(Self {
            name: read.read_str()?,
            value: read.read_str()?,
            signature: read.read_optional(ser::NetworkReadExt::read_str)?,
        })
    }
    pub fn write(&self, write: &mut impl ser::NetworkWriteExt) -> Result<(), ser::WriteError> {
        write.write_string(&self.name)?;
        write.write_string(&self.value)?;
        write.write_optional(&self.signature, |w, v| w.write_string(v))?;
        Ok(())
    }
}
