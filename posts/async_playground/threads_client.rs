use std::io;
use std::io::prelude::*;
use std::net::TcpStream;
use std::thread;

fn foo_request() -> io::Result<()> {
    let socket = TcpStream::connect("localhost:8000")?;
    // io::copy(&mut socket, &mut io::stdout()) is similar, but println is more likely to keep each
    // line intact when there are many client printing in parallel.
    for line in io::BufReader::new(socket).lines() {
        println!("{}", line?);
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let mut client_handles = Vec::new();
    for _ in 1..=10 {
        client_handles.push(thread::spawn(foo_request));
    }
    for handle in client_handles {
        handle.join().unwrap()?;
    }
    Ok(())
}
