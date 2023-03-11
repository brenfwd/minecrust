use async_std::{
    io::{ReadExt, WriteExt},
    net::{TcpListener, TcpStream},
    task,
};

fn read_byte(buf: &mut Vec<u8>) -> u8 {
    assert!(buf.len() > 0);
    buf.remove(0)
}

fn read_var_int(buf: &mut Vec<u8>) -> i32 {
    let mut value: i32 = 0;
    let mut position = 0;
    let mut current_byte;

    loop {
        current_byte = read_byte(buf);
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

fn read_string(buf: &mut Vec<u8>) -> String {
    let length = read_var_int(buf);
    let mut bytes = vec![0; length as usize];
    for i in 0..length {
        bytes[i as usize] = read_byte(buf);
    }
    String::from_utf8(bytes).unwrap()
}

fn read_ushort(buf: &mut Vec<u8>) -> u16 {
    let mut bytes = [0; 2];
    bytes[0] = read_byte(buf);
    bytes[1] = read_byte(buf);
    u16::from_be_bytes(bytes)
}

#[derive(Debug)]
enum ClientState {
    Handshaking,
    Play,
    Status,
    Login,
}

struct Client {
    buf: Vec<u8>,
    stream: TcpStream,
    state: ClientState,
}

impl Client {
    pub fn new(stream: TcpStream) -> Client {
        Client {
            buf: vec![],
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
            self.buf.extend_from_slice(&frame[..bytes_read]);
            println!(
                "\tRead {} bytes into vec, buf.len() = {}",
                bytes_read,
                self.buf.len()
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
        let length = read_var_int(&mut self.buf);

        assert!(length >= 0);
        dbg!(&self.buf);

        let remaining = self.buf.split_off(length as usize);
        let mut payload = self.buf.clone();
        self.buf = remaining;

        dbg!(length, &payload);

        let packet_id = read_var_int(&mut payload);
        dbg!(packet_id, &payload);

        match (&self.state, packet_id) {
            (ClientState::Handshaking, 0x00) => {
                let protocol_version = read_var_int(&mut payload);
                let server_address = read_string(&mut payload);
                let server_port = read_ushort(&mut payload);
                let next_state = read_var_int(&mut payload);
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
