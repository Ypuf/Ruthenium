use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sock = TcpListener::bind("0.0.0.0:25565").await?;

    loop {
        let (mut socket, _) = sock.accept().await?;

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
            }
        });
    }
}
