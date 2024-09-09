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

fn server_main(listener: TcpListener) -> io::Result<()> {
    let mut n = 1;
    loop {
        let (socket, _) = listener.accept()?;
        thread::spawn(move || foo_response(n, socket).unwrap());
        n += 1;
    }
}

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
    // Open the listener here, to avoid racing against the server thread.
    let listener = TcpListener::bind("localhost:8000")?;
    thread::spawn(|| server_main(listener).unwrap());
    let mut client_handles = Vec::new();
    for _ in 1..=10 {
        client_handles.push(thread::spawn(foo_request));
    }
    for handle in client_handles {
        handle.join().unwrap()?;
    }
    Ok(())
}
