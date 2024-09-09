use std::io;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn foo_response(n: u64, mut socket: TcpStream) -> io::Result<()> {
    socket.write_all(format!("start {n}\n").as_bytes()).await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    socket.write_all(format!("end {n}\n").as_bytes()).await?;
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
    let socket = TcpStream::connect("localhost:8000").await?;
    // tokio::io::copy(&mut socket, &mut tokio::io::stdout()) is similar, but println is more
    // likely to keep each line intact when there are many client printing in parallel.
    let mut lines = tokio::io::BufReader::new(socket).lines();
    while let Some(line) = lines.next_line().await? {
        println!("{}", line);
    }
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
