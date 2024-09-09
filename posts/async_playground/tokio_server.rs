use std::io;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

async fn foo_response(n: u64, mut socket: TcpStream) -> io::Result<()> {
    socket.write_all(format!("start {n}\n").as_bytes()).await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    socket.write_all(format!("end {n}\n").as_bytes()).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("localhost:8000").await?;
    let mut n = 1;
    loop {
        let (socket, _) = listener.accept().await?;
        tokio::task::spawn(async move { foo_response(n, socket).await.unwrap() });
        n += 1;
    }
}
