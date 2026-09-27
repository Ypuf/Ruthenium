use tokio::net::{TcpListener, TcpStream};

enum ConnectionState {
    Handshaking,
    Play,
    Status,
    Login,
    Configuration,
}

pub struct Connection {
    pub state: ConnectionState,
    pub stream: &TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            state: ConnectionState::Handshaking,
        }
    }

    fn handlePacket(&self) -> _ {
        let (tcp_reader, tcp_writer) = self.stream.into_split();
        // tcp_reader.
    }
}
