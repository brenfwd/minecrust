use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum BufferError {
    #[error("out of bounds (want {want} bytes, remaining {remaining})")]
    OutOfBounds { want: usize, remaining: usize },
    #[error("invalid varint position ({position} > {limit})")]
    InvalidVarIntSize { position: usize, limit: usize },
    #[error("invalid string length ({0})")]
    InvalidStringLength(i32),
    #[error("invalid UTF-8 string")]
    InvalidUTF8String,
}

pub(crate) type BufferResult<T> = Result<T, BufferError>;

pub(crate) struct Buffer {
    data: Vec<u8>,
}

#[allow(dead_code)]
impl Buffer {
    pub fn new() -> Buffer {
        Buffer { data: vec![] }
    }

    pub fn from_vec(v: Vec<u8>) -> Buffer {
        Buffer { data: v }
    }

    pub fn remaining(&self) -> usize {
        self.data.len()
    }

    pub fn push_byte(&mut self, byte: u8) -> &mut Self {
        self.data.push(byte);
        self
    }

    pub fn push_slice(&mut self, slice: &[u8]) -> &mut Self {
        self.data.extend_from_slice(slice);
        self
    }

    fn check_bytes(&mut self, want: usize) -> BufferResult<()> {
        let remaining = self.remaining();
        if remaining < want {
            Err(BufferError::OutOfBounds { want, remaining })
        } else {
            Ok(())
        }
    }

    pub fn read_u8(&mut self) -> BufferResult<u8> {
        self.check_bytes(1)?;
        Ok(self.data.remove(0))
    }

    pub fn read_var_int(&mut self) -> BufferResult<i32> {
        let mut value: i32 = 0;
        let mut position = 0;
        let mut current_byte;

        loop {
            current_byte = self.read_u8()?;
            // TODO: see if this works with signed/unsigned numbers properly
            value |= ((current_byte & 0b01111111) as i32) << position; // last 7 bits
            if (current_byte & 0b10000000) == 0 {
                // first bit (stop bit)
                break;
            }
            position += 7;
            if position >= 32 {
                return Err(BufferError::InvalidVarIntSize {
                    position,
                    limit: 32,
                });
            }
        }

        Ok(value)
    }

    pub fn read_bytes_into(&mut self, into: &mut [u8]) -> BufferResult<()> {
        let len = into.len();
        self.check_bytes(len)?;
        for i in 0..len {
            into[i] = self.read_u8()?;
        }
        Ok(())
    }

    pub fn read_bytes(&mut self, length: usize) -> BufferResult<Vec<u8>> {
        self.check_bytes(length)?;
        let mut bytes = vec![0; length];
        self.read_bytes_into(&mut bytes)?;
        Ok(bytes)
    }

    pub fn read_buffer(&mut self, length: usize) -> BufferResult<Buffer> {
        Ok(Buffer::from_vec(self.read_bytes(length)?))
    }

    pub fn read_string(&mut self) -> BufferResult<String> {
        let length = self.read_var_int()?;
        if length < 0 {
            return Err(BufferError::InvalidStringLength(length));
        }
        let bytes = self.read_bytes(length as usize)?;
        String::from_utf8(bytes).map_err(|_| BufferError::InvalidUTF8String)
    }

    pub fn read_ushort(&mut self) -> BufferResult<u16> {
        let mut bytes = [0; 2];
        self.read_bytes_into(&mut bytes)?;
        Ok(u16::from_be_bytes(bytes))
    }
}
