use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

mod lib;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sock = TcpListener::bind("127.0.0.1:25565").await?;

    loop {
        let (mut socket, _) = sock.accept().await?;
        let mut connection = Connection::new(socket);

        tokio::spawn(async move {
            let mut buf = [0; 1024];

            loop {
                let _n = match socket.read(&mut buf).await {
                    Ok(0) => return,
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("failed to read: {}", e);
                        return;
                    }
                };

                println!("read {} bytes", _n);
                println!("buf: {:?}", &buf[.._n]);
                println!("buf str: {}", String::from_utf8_lossy(&buf[.._n]))
            }
        });
    }
}
