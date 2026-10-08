use std::any::Any;

use crate::codec::var_int::VarIntType;

pub trait Packet {
    const PACKET_ID: VarIntType;
}

pub trait MultiVersionPacket {
    fn to_id(version: dyn Any) -> i32;
}
