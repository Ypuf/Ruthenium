use aes::cipher::KeyIvInit;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::{
    Aes128Cfb8Enc, CompressionLevel, CompressionThreshold, MAX_PACKET_SIZE, PacketEncodeError,
    StreamEncryptor, codec::var_int::VarInt,
};

pub enum EncryptedWriter<W: AsyncWrite + Unpin> {
    Encrypt(Box<StreamEncryptor<W>>),
    None(W),
}

impl<W: AsyncWrite + Unpin> EncryptedWriter<W> {
    pub fn upgrade(self, cipher: Aes128Cfb8Enc) -> Self {
        match self {
            EncryptedWriter::None(stream) => {
                Self::Encrypt(Box::new(StreamEncryptor::new(cipher, stream)))
            }
            EncryptedWriter::Encrypt(_) => self,
        }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for EncryptedWriter<W> {
    #[inline]
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Self::Encrypt(writer) => {
                let writer = std::pin::Pin::new(writer);
                writer.poll_write(cx, buf)
            }
            Self::None(writer) => {
                let writer = std::pin::Pin::new(writer);
                writer.poll_write(cx, buf)
            }
        }
    }

    #[inline]
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Encrypt(writer) => {
                let writer = std::pin::Pin::new(writer);
                writer.poll_flush(cx)
            }
            Self::None(writer) => {
                let writer = std::pin::Pin::new(writer);
                writer.poll_flush(cx)
            }
        }
    }

    #[inline]
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Encrypt(writer) => {
                let writer = std::pin::Pin::new(writer);
                writer.poll_shutdown(cx)
            }
            Self::None(writer) => {
                let writer = std::pin::Pin::new(writer);
                writer.poll_shutdown(cx)
            }
        }
    }
}

pub struct TCPNetworkEncoder<W: AsyncWrite + Unpin> {
    writer: Option<EncryptedWriter<W>>,
    compression: Option<(CompressionThreshold, CompressionLevel)>,
    frame_scratch: Vec<u8>,
}

impl<W: AsyncWrite + Unpin> TCPNetworkEncoder<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: Some(EncryptedWriter::None(writer)),
            compression: None,
            frame_scratch: Vec::new(),
        }
    }

    pub const fn set_compression(
        &mut self,
        threshold: CompressionThreshold,
        level: CompressionLevel,
    ) {
        self.compression = Some((threshold, level));
    }

    pub fn set_encryption(&mut self, key: &[u8; 16]) -> Result<(), PacketEncodeError> {
        if matches!(self.writer, Some(EncryptedWriter::Encrypt(_))) {
            return Err(PacketEncodeError::Message("Encryption already set".into()));
        }
        let cipher = Aes128Cfb8Enc::new_from_slices(key, key)
            .map_err(|_| PacketEncodeError::Message("Invalid key".into()))?;

        if let Some(writer) = self.writer.take() {
            self.writer = Some(writer.upgrade(cipher));
        }

        Ok(())
    }

    pub async fn write_packet(&mut self, id: i32, payload: &[u8]) -> Result<(), PacketEncodeError> {
        let packet_id = VarInt(id);
        let packet_data_len = packet_id.written_size() + payload.len();

        self.frame_scratch.clear();

        match self.compression {
            Some((threshold, level)) if packet_data_len >= threshold => {
                self.write_compressed_frame(packet_id, payload, level)?;
            }
            Some(_) => {
                let data_len = VarInt(0);
                let packet_len = data_len.written_size() + packet_data_len;
                Self::check_packet_len(packet_len)?;

                Self::encode_var_int(&VarInt(packet_len as i32), &mut self.frame_scratch)?;
                Self::encode_var_int(&data_len, &mut self.frame_scratch)?;
                Self::encode_var_int(&packet_id, &mut self.frame_scratch)?;
                self.frame_scratch.extend_from_slice(payload);
            }
            None => {
                Self::check_packet_len(packet_data_len)?;

                Self::encode_var_int(&VarInt(packet_data_len as i32), &mut self.frame_scratch)?;
                Self::encode_var_int(&packet_id, &mut self.frame_scratch)?;
                self.frame_scratch.extend_from_slice(payload);
            }
        }

        let stream_writer = self
            .writer
            .as_mut()
            .ok_or_else(|| PacketEncodeError::Message("Writer missing".into()))?;

        stream_writer
            .write_all(&self.frame_scratch)
            .await
            .map_err(|err| PacketEncodeError::Message(err.to_string()))?;
        stream_writer
            .flush()
            .await
            .map_err(|err| PacketEncodeError::Message(err.to_string()))?;

        Ok(())
    }

    fn write_compressed_frame(
        &mut self,
        _packet_id: VarInt,
        _payload: &[u8],
        _level: CompressionLevel,
    ) -> Result<(), PacketEncodeError> {
        Err(PacketEncodeError::CompressionFailed(
            "Compression is not implemented yet".into(),
        ))
    }

    fn check_packet_len(packet_len: usize) -> Result<(), PacketEncodeError> {
        if packet_len as u64 > MAX_PACKET_SIZE {
            return Err(PacketEncodeError::TooLong(packet_len));
        }
        Ok(())
    }

    fn encode_var_int(value: &VarInt, buf: &mut Vec<u8>) -> Result<(), PacketEncodeError> {
        value
            .encode(buf)
            .map_err(|err| PacketEncodeError::Message(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java::packet_decoder::TCPNetworkDecoder;

    #[tokio::test]
    async fn encode_without_compression_and_encryption() -> Result<(), Box<dyn std::error::Error>> {
        let mut encoder = TCPNetworkEncoder::new(Vec::new());
        encoder.write_packet(1, b"mrow").await?;

        let Some(EncryptedWriter::None(written)) = encoder.writer else {
            panic!("Writer should be unencrypted");
        };
        assert_eq!(written, [&[5, 1][..], b"mrow"].concat());
        Ok(())
    }

    #[tokio::test]
    async fn encode_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let mut encoder = TCPNetworkEncoder::new(Vec::new());
        encoder.write_packet(1, b"mrow").await?;
        encoder.write_packet(300, b"").await?;

        let Some(EncryptedWriter::None(written)) = encoder.writer else {
            panic!("Writer should be unencrypted");
        };

        let mut decoder = TCPNetworkDecoder::new(written.as_slice());
        let p1 = decoder.get_raw_packet().await?;
        assert_eq!(p1.id, 1);
        assert_eq!(p1.payload.as_ref(), b"mrow");

        let p2 = decoder.get_raw_packet().await?;
        assert_eq!(p2.id, 300);
        assert!(p2.payload.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn encode_below_compression_threshold() -> Result<(), Box<dyn std::error::Error>> {
        let mut encoder = TCPNetworkEncoder::new(Vec::new());
        encoder.set_compression(1000, 6);
        encoder.write_packet(2, b"mrow, compression!").await?;

        let Some(EncryptedWriter::None(written)) = encoder.writer else {
            panic!("Writer should be unencrypted");
        };

        let mut decoder = TCPNetworkDecoder::new(written.as_slice());
        decoder.set_compression(1000);
        let raw_packet = decoder.get_raw_packet().await?;
        assert_eq!(raw_packet.id, 2);
        assert_eq!(raw_packet.payload.as_ref(), b"mrow, compression!");
        Ok(())
    }

    #[tokio::test]
    async fn encode_above_compression_threshold_errors() {
        let mut encoder = TCPNetworkEncoder::new(Vec::new());
        encoder.set_compression(4, 6);

        let result = encoder.write_packet(2, b"Hello").await;
        assert!(matches!(
            result,
            Err(PacketEncodeError::CompressionFailed(_))
        ));
    }

    #[tokio::test]
    async fn encode_with_encryption() -> Result<(), Box<dyn std::error::Error>> {
        let key = [0x01u8; 16];

        let mut encoder = TCPNetworkEncoder::new(Vec::new());
        encoder.set_encryption(&key)?;
        encoder.write_packet(3, b"mroww, encrypted!").await?;
        encoder.write_packet(4, b"second packet").await?;

        let Some(EncryptedWriter::Encrypt(written)) = encoder.writer else {
            panic!("Writer should be encrypted");
        };
        let written = written.into_inner();
        assert_ne!(&written[2..], b"mroww, encrypted!");

        let mut decoder = TCPNetworkDecoder::new(written.as_slice());
        decoder.set_encryption(&key)?;
        let p1 = decoder.get_raw_packet().await?;
        assert_eq!(p1.id, 3);
        assert_eq!(p1.payload.as_ref(), b"mroww, encrypted!");

        let p2 = decoder.get_raw_packet().await?;
        assert_eq!(p2.id, 4);
        assert_eq!(p2.payload.as_ref(), b"second packet");
        Ok(())
    }

    #[test]
    fn encryption_already_set() -> Result<(), Box<dyn std::error::Error>> {
        let key = [0x00u8; 16];
        let mut encoder = TCPNetworkEncoder::new(Vec::new());
        encoder.set_encryption(&key)?;
        assert!(encoder.set_encryption(&key).is_err());
        Ok(())
    }

    #[tokio::test]
    async fn encode_too_long() {
        let mut encoder = TCPNetworkEncoder::new(Vec::new());
        let payload = vec![0u8; MAX_PACKET_SIZE as usize];

        let result = encoder.write_packet(1, &payload).await;
        assert!(matches!(result, Err(PacketEncodeError::TooLong(_))));
    }
}
