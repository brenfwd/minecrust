use thiserror::Error;

use crate::buffer::{Buffer, BufferError, FromBuffer, ToBuffer};

#[derive(Debug)]
#[allow(dead_code)]
pub struct C2SHandshakePacket {
    pub protocol_version: i32,
    pub server_address: String,
    pub server_port: u16,
    pub next_state: i32,
}

impl crate::FromBuffer for C2SHandshakePacket {
    fn from_buffer(buf: &mut crate::buffer::Buffer) -> Result<Self, crate::buffer::BufferError> {
        let protocol_version = buf.read_var_int()?;
        let server_address = buf.read_string()?;
        let server_port = buf.read_u16()?;
        let next_state = buf.read_var_int()?;
        Ok(C2SHandshakePacket {
            protocol_version,
            server_address,
            server_port,
            next_state,
        })
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct S2CStatusResponsePacket {
    pub json_response: String,
}

impl crate::buffer::ToBuffer for S2CStatusResponsePacket {
    fn to_buffer(&self, buf: &mut crate::buffer::Buffer) {
        buf.write_var_int(0x00).write_string(&self.json_response);
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct S2CPingPacket {
    pub payload: i64,
}

impl crate::buffer::ToBuffer for S2CPingPacket {
    fn to_buffer(&self, buf: &mut crate::buffer::Buffer) {
        buf.write_var_int(0x01).write_i64(self.payload);
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct C2SPingPacket {
    pub payload: i64,
}

impl crate::buffer::FromBuffer for C2SPingPacket {
    fn from_buffer(buf: &mut crate::buffer::Buffer) -> Result<Self, crate::buffer::BufferError> {
        let payload = buf.read_i64()?;
        Ok(C2SPingPacket { payload })
    }
}
