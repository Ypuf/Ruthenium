use std::io::{Error, Write};

use crate::serial::PacketWrite;

impl PacketWrite for bool {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&if *self { [1] } else { [0] }).map(|_| ())
    }
}

impl PacketWrite for i8 {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_le_bytes()).map(|_| ())
    }
}

impl PacketWrite for i16 {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_le_bytes()).map(|_| ())
    }
}

impl PacketWrite for i32 {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_le_bytes()).map(|_| ())
    }

    fn write_be<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_be_bytes()).map(|_| ())
    }
}

impl PacketWrite for i64 {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_le_bytes()).map(|_| ())
    }

    fn write_be<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_be_bytes()).map(|_| ())
    }
}

impl PacketWrite for u8 {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_le_bytes()).map(|_| ())
    }
}

impl PacketWrite for u16 {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_le_bytes()).map(|_| ())
    }

    fn write_be<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_be_bytes()).map(|_| ())
    }
}

impl PacketWrite for u32 {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_le_bytes()).map(|_| ())
    }

    fn write_be<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_be_bytes()).map(|_| ())
    }
}

impl PacketWrite for u64 {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_le_bytes()).map(|_| ())
    }

    fn write_be<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_be_bytes()).map(|_| ())
    }
}

impl PacketWrite for f32 {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_le_bytes()).map(|_| ())
    }
}

impl PacketWrite for f64 {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write(&self.to_le_bytes()).map(|_| ())
    }
}

impl<T: PacketWrite, const N: usize> PacketWrite for [T; N] {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        for item in self {
            item.write(writer)?;
        }
        Ok(())
    }
}

impl PacketWrite for String {
    fn write<W: Write>(&self, _writer: &mut W) -> Result<(), Error> {
        todo!("String::write")
    }
}
