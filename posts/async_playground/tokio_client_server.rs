use std::io;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

async fn one_response(n: u64, mut socket: TcpStream) -> io::Result<()> {
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
        tokio::task::spawn(async move { one_response(n, socket).await.unwrap() });
        n += 1;
    }
}

async fn client_main() -> io::Result<()> {
    let mut socket = TcpStream::connect("localhost:8000").await?;
    tokio::io::copy(&mut socket, &mut tokio::io::stdout()).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // Open the listener first, to avoid racing against the server thread.
    let listener = TcpListener::bind("0.0.0.0:8000").await?;
    // Start the server on a background task.
    tokio::task::spawn(async { server_main(listener).await.unwrap() });
    // Run ten clients as ten different tasks.
    let mut client_handles = Vec::new();
    for _ in 1..=10 {
        client_handles.push(tokio::task::spawn(client_main()));
    }
    for handle in client_handles {
        handle.await.unwrap()?;
    }
    Ok(())
}
