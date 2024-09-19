use std::io;
use std::io::prelude::*;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8000")?;
    let mut n = 1;
    loop {
        let (mut socket, _) = listener.accept()?;
        writeln!(&mut socket, "start {n}")?;
        thread::sleep(Duration::from_secs(1));
        writeln!(&mut socket, "end {n}")?;
        n += 1;
    }
}
