use async_std::{
    io::{ReadExt, WriteExt},
    net::{TcpListener, TcpStream},
    task,
};

struct Buffer {
    data: Vec<u8>,
}

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

    // pub fn push_byte(&mut self, byte: u8) -> &mut Self {
    //     self.data.push(byte);
    //     self
    // }

    pub fn push_slice(&mut self, slice: &[u8]) -> &mut Self {
        self.data.extend_from_slice(slice);
        self
    }

    pub fn read_u8(&mut self) -> u8 {
        assert!(self.remaining() > 0);
        self.data.remove(0)
    }

    pub fn read_var_int(&mut self) -> i32 {
        let mut value: i32 = 0;
        let mut position = 0;
        let mut current_byte;

        loop {
            current_byte = self.read_u8();
            // TODO: see if this works with signed/unsigned numbers properly
            value |= ((current_byte & 0b01111111) as i32) << position; // last 7 bits
            if (current_byte & 0b10000000) == 0 {
                // first bit (stop bit)
                break;
            }
            position += 7;
            assert!(position < 32);
        }

        value
    }

    pub fn read_bytes(&mut self, into: &mut [u8]) {
        let len = into.len();
        assert!(self.remaining() >= len);
        for i in 0..len {
            into[i] = self.read_u8();
        }
    }

    pub fn read_buffer(&mut self, length: usize) -> Buffer {
        let mut bytes = vec![0; length];
        self.read_bytes(&mut bytes);
        Buffer::from_vec(bytes)
    }

    pub fn read_string(&mut self) -> String {
        let length = self.read_var_int();
        assert!(length >= 0);
        let mut bytes = vec![0; length as usize];
        self.read_bytes(&mut bytes);
        String::from_utf8(bytes).unwrap()
    }

    pub fn read_ushort(&mut self) -> u16 {
        let mut bytes = [0; 2];
        self.read_bytes(&mut bytes);
        u16::from_be_bytes(bytes)
    }
}

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

    pub async fn handle(&mut self) {
        let mut frame = [0; 1024];
        loop {
            let bytes_read = match self.stream.read(&mut frame).await {
                Ok(n) => {
                    if n == 0 {
                        println!("\tEnd of stream!");
                        return;
                    }
                    n
                }
                Err(e) => {
                    println!("\tError: {e}");
                    return;
                }
            };
            // append frame bytes into self.buf:
            self.buf.push_slice(&frame[..bytes_read]);
            println!(
                "\tRead {} bytes into buf, buf.remaining() = {}",
                bytes_read,
                self.buf.remaining()
            );

            // handle the packets
            self.process_packets();
        }
    }

    pub fn get_addr(&self) -> String {
        self.stream.peer_addr().unwrap().to_string()
    }

    fn process_packets(&mut self) {
        // Each packet:
        //      Length      VarInt
        //      PacketID    VarInt
        //      Data        Bytes...

        let length = self.buf.read_var_int();
        assert!(length >= 0);

        let mut payload = self.buf.read_buffer(length as usize);

        let packet_id = payload.read_var_int();

        dbg!(packet_id);

        match (&self.state, packet_id) {
            (ClientState::Handshaking, 0x00) => {
                let protocol_version = payload.read_var_int();
                let server_address = payload.read_string();
                let server_port = payload.read_ushort();
                let next_state = payload.read_var_int();
                dbg!(protocol_version, server_address, server_port, next_state);
                match next_state {
                    1 => self.state = ClientState::Status,
                    2 => self.state = ClientState::Login,
                    _ => {
                        eprintln!("Invalid next_state: {next_state}");
                        return;
                    }
                }
            }
            _ => {
                eprintln!("\tUnhandled packet ID: {:?}/{packet_id}", &self.state);
            }
        }
    }
}

async fn handle_client(stream: TcpStream) {
    let mut client = Client::new(stream);
    println!("New client connected: {}", client.get_addr());

    client.handle().await;

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
