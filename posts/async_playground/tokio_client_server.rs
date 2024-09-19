use std::io;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

async fn foo_response(n: u64, mut socket: TcpStream) -> io::Result<()> {
    let start_msg = format!("start {n}\n");
    socket.write_all(start_msg.as_bytes()).await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let end_msg = format!("end {n}\n");
    socket.write_all(end_msg.as_bytes()).await?;
    Ok(())
}

async fn server_main(listener: TcpListener) -> io::Result<()> {
    let mut n = 1;
    loop {
        let (socket, _) = listener.accept().await?;
        tokio::task::spawn(async move { foo_response(n, socket).await.unwrap() });
        n += 1;
    }
}

async fn foo_request() -> io::Result<()> {
    let mut socket = TcpStream::connect("localhost:8000").await?;
    tokio::io::copy(&mut socket, &mut tokio::io::stdout()).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // Open the listener here, to avoid racing against the server thread.
    let listener = TcpListener::bind("localhost:8000").await?;
    tokio::task::spawn(async { server_main(listener).await.unwrap() });
    let mut client_handles = Vec::new();
    for _ in 1..=10 {
        client_handles.push(tokio::task::spawn(foo_request()));
    }
    for handle in client_handles {
        handle.await.unwrap()?;
    }
    Ok(())
}
