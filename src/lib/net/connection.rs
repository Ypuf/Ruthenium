use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

enum ConnectionState {
    Handshaking,
    Play,
    Status,
    Login,
    Configuration,
}

pub struct Connection {
    pub state: ConnectionState,
    pub reader: OwnedReadHalf,
    pub writer: OwnedWriteHalf,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        let (reader, writer) = stream.into_split();

        Self {
            reader,
            writer,
            state: ConnectionState::Handshaking,
        }
    }

    fn handle_packet(&mut self) -> Result<(), ()> {
        // let (tcp_reader, tcp_writer) = self.stream.into_split();
        // tcp_reader.
        Ok(())
    }
}
