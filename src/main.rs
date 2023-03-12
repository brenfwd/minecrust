mod buffer;

use async_std::{
    io::{self, ReadExt, WriteExt},
    net::{TcpListener, TcpStream},
    task,
};
use thiserror::Error;

use buffer::{Buffer, BufferError, FromBuffer, ToBuffer};

// region: packets

#[derive(Debug)]
#[allow(dead_code)]
struct C2SHandshakePacket {
    protocol_version: i32,
    server_address: String,
    server_port: u16,
    next_state: i32,
}

impl FromBuffer for C2SHandshakePacket {
    fn from_buffer(buf: &mut Buffer) -> Result<Self, BufferError> {
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
struct S2CStatusResponsePacket {
    json_response: String,
}

impl ToBuffer for S2CStatusResponsePacket {
    fn to_buffer(&self, buf: &mut Buffer) {
        buf.write_var_int(0x00).write_string(&self.json_response);
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct S2CPingPacket {
    payload: i64,
}

impl ToBuffer for S2CPingPacket {
    fn to_buffer(&self, buf: &mut Buffer) {
        buf.write_var_int(0x01).write_i64(self.payload);
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct C2SPingPacket {
    payload: i64,
}

impl FromBuffer for C2SPingPacket {
    fn from_buffer(buf: &mut Buffer) -> Result<Self, BufferError> {
        let payload = buf.read_i64()?;
        Ok(C2SPingPacket { payload })
    }
}

// endregion: packets

// region: client

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
            self.process_packets().await?;
        }
    }

    pub fn get_addr(&self) -> String {
        self.stream.peer_addr().unwrap().to_string()
    }

    async fn process_packets(&mut self) -> ClientResult<()> {
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
                let packet = C2SHandshakePacket::from_buffer(&mut payload)?;
                dbg!(&packet);
                match packet.next_state {
                    1 => self.state = ClientState::Status,
                    2 => self.state = ClientState::Login,
                    _ => {
                        return Err(ClientError::LogicError(format!(
                            "Invalid next_state: {} (from {:?})",
                            packet.next_state, self.state
                        )));
                    }
                }
                return Ok(());
            }
            (ClientState::Status, 0x00) => {
                let packet = S2CStatusResponsePacket {
                    json_response: r#"{"version":{"name":"1.8.9","protocol":47},"players":{"max":20,"online":0},"description":{"text":"A Minecrust Server"}}"#.to_string(),
                };
                let mut buf = Buffer::new();
                packet.to_buffer(&mut buf);
                let mut len_buf = Buffer::new();
                len_buf.write_var_int(buf.remaining() as i32);
                buf.prepend_buffer(&mut len_buf);
                self.stream
                    .write_all(&buf.read_bytes(buf.remaining())?)
                    .await?;
                return Ok(());
            }
            (ClientState::Status, 0x01) => {
                let packet = C2SPingPacket::from_buffer(&mut payload)?;
                dbg!(&packet);
                let packet = S2CPingPacket {
                    payload: packet.payload,
                };
                let mut buf = Buffer::new();
                packet.to_buffer(&mut buf);
                let mut len_buf = Buffer::new();
                len_buf.write_var_int(buf.remaining() as i32);
                buf.prepend_buffer(&mut len_buf);
                self.stream
                    .write_all(&buf.read_bytes(buf.remaining())?)
                    .await?;
                return Ok(());
            }
            _ => {
                eprintln!("\tUnhandled packet ID: {:?}/{packet_id}", &self.state);
                return Ok(());
            }
        }
    }
}

// endregion: client

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
