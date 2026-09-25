use tokio::net::{TcpListener, TcpStream};

pub fn handle_connection(tcp_stream: TcpStream) {
    let (tcp_reader, tcp_writer) = tcp_stream.into_split();
}
