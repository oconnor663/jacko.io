use std::io;
use std::io::prelude::*;
use std::net::TcpStream;

fn main() -> io::Result<()> {
    let socket = TcpStream::connect("localhost:8000")?;
    // io::copy(&mut socket, &mut io::stdout()) is similar, but println is more likely to keep each
    // line intact when there are many client printing in parallel.
    for line in io::BufReader::new(socket).lines() {
        println!("{}", line?);
    }
    Ok(())
}
