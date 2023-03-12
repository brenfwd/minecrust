use async_std::{
    io::{self, ReadExt},
    net::{TcpListener, TcpStream},
    task,
};

use thiserror::Error;

#[derive(Error, Debug)]
enum BufferError {
    #[error("out of bounds (want {want} bytes, remaining {remaining})")]
    OutOfBounds { want: usize, remaining: usize },
    #[error("invalid varint position ({position} > {limit})")]
    InvalidVarIntSize { position: usize, limit: usize },
    #[error("invalid string length ({0})")]
    InvalidStringLength(i32),
    #[error("invalid UTF-8 string")]
    InvalidUTF8String,
}

type BufferResult<T> = Result<T, BufferError>;

struct Buffer {
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

#[derive(Debug, Error)]
enum ClientError {
    #[error("packet decode error: {0:?}")]
    PacketDecodeError(#[from] BufferError),
    #[error("logic error: ({0})")]
    LogicError(String),
    #[error("network error: {0:?}")]
    NetworkError(#[from] io::Error),
}

type ClientResult<T> = Result<T, ClientError>;

#[allow(dead_code)]
#[derive(Debug)]
enum ClientState {
    Handshaking,
    Play,
    Status,
    Login,
}

struct Client {
    buf: Buffer,
    stream: TcpStream,
    state: ClientState,
}

impl Client {
    pub fn new(stream: TcpStream) -> Client {
        Client {
            buf: Buffer::new(),
            stream,
            state: ClientState::Handshaking,
        }
    }

    pub async fn handle(&mut self) -> ClientResult<()> {
        let mut frame = [0; 1024];
        loop {
            let bytes_read = self.stream.read(&mut frame).await?;
            if bytes_read == 0 {
                println!("End of stream!");
                return Ok(());
            }
            // append frame bytes into self.buf:
            self.buf.push_slice(&frame[..bytes_read]);
            println!(
                "\tRead {} bytes into buf, buf.remaining() = {}",
                bytes_read,
                self.buf.remaining()
            );

            // handle the packets
            self.process_packets()?;
        }
    }

    pub fn get_addr(&self) -> String {
        self.stream.peer_addr().unwrap().to_string()
    }

    fn process_packets(&mut self) -> ClientResult<()> {
        // Each packet:
        //      Length      VarInt
        //      PacketID    VarInt
        //      Data        Bytes...

        let length = self.buf.read_var_int()?;
        assert!(length >= 0);

        let mut payload = self.buf.read_buffer(length as usize)?;

        let packet_id = payload.read_var_int()?;

        dbg!(packet_id);

        match (&self.state, packet_id) {
            (ClientState::Handshaking, 0x00) => {
                let protocol_version = payload.read_var_int()?;
                let server_address = payload.read_string()?;
                let server_port = payload.read_ushort()?;
                let next_state = payload.read_var_int()?;
                dbg!(protocol_version, server_address, server_port, next_state);
                match next_state {
                    1 => self.state = ClientState::Status,
                    2 => self.state = ClientState::Login,
                    _ => {
                        return Err(ClientError::LogicError(format!(
                            "Invalid next_state: {next_state} (from {:?}",
                            self.state
                        )));
                    }
                }
                return Ok(());
            }
            _ => {
                eprintln!("\tUnhandled packet ID: {:?}/{packet_id}", &self.state);
                return Ok(());
            }
        }
    }
}

async fn handle_client(stream: TcpStream) {
    let mut client = Client::new(stream);
    println!("New client connected: {}", client.get_addr());

    match client.handle().await {
        Err(error) => {
            eprintln!(
                "An error occurred in the client connection for {}: {:?}",
                client.get_addr(),
                error
            );
        }
        _ => {}
    }

    println!("Client going out of scope: {}\n", client.get_addr());
}

async fn run_server() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:25565").await?;
    loop {
        let (stream, _) = listener.accept().await?;
        task::spawn(handle_client(stream));
    }
}

#[async_std::main]
async fn main() {
    println!("Welcome to Minecrust v0.0.1");
    println!("Starting server on 0.0.0.0:25565");
    run_server().await.unwrap();
}
