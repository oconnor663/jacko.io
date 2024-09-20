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
        // Using format! instead of write! avoids breaking up lines across multiple writes. This is
        // easier than doing line buffering on the client side.
        let start_msg = format!("start {n}\n");
        socket.write_all(start_msg.as_bytes())?;
        thread::sleep(Duration::from_secs(1));
        let end_msg = format!("end {n}\n");
        socket.write_all(end_msg.as_bytes())?;
        n += 1;
    }
}
