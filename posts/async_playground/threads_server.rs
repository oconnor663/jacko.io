use std::io;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

fn foo_response(n: u64, mut socket: TcpStream) -> io::Result<()> {
    writeln!(&mut socket, "start {n}")?;
    thread::sleep(Duration::from_secs(1));
    writeln!(&mut socket, "end {n}")?;
    Ok(())
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("localhost:8000")?;
    let mut n = 1;
    loop {
        let (socket, _) = listener.accept()?;
        thread::spawn(move || foo_response(n, socket).unwrap());
        n += 1;
    }
}
