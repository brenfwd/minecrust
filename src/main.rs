mod buffer;

use async_std::{
    io::{self, ReadExt},
    net::{TcpListener, TcpStream},
    task,
};
use thiserror::Error;

use buffer::{Buffer, BufferError};

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
