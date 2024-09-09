use std::io;
use tokio::io::AsyncBufReadExt;
use tokio::net::TcpStream;

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
    let mut client_handles = Vec::new();
    for _ in 1..=10 {
        client_handles.push(tokio::task::spawn(foo_request()));
    }
    for handle in client_handles {
        handle.await.unwrap()?;
    }
    Ok(())
}
