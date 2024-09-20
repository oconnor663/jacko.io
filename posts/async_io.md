# Async Rust, Part Three: IO
###### \[DRAFT]

- [Introduction](async_intro.html)
- [Part One: Futures](async_futures.html)
- [Part Two: Tasks](async_tasks.html)
- Part Three: IO (you are here)

Of course, efficient sleeping isn't why async/await was invented. The goal all
along has been efficient IO, especially network IO. Now that we understand
futures and tasks, we have all the tools we need to do some real work.

Here's a toy server program:

```rust
LINK: Playground playground://async_playground/single_threaded_server.rs
fn main() -> io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8000")?;
    let mut n = 1;
    loop {
        let (mut socket, _) = listener.accept()?;
        let start_msg = format!("start {n}\n");
        socket.write_all(start_msg.as_bytes())?;
        thread::sleep(Duration::from_secs(1));
        let end_msg = format!("end {n}\n");
        socket.write_all(end_msg.as_bytes())?;
        n += 1;
    }
}
```

This program starts listening on port 8000.[^zero_ip] For each connection it
receives, it writes a start message, sleeps for one second, and writes an end
message.[^writeln] Here's a toy client for our toy server:

[^zero_ip]: `0.0.0.0` is the special IP address that means "all IPv4 interfaces
    on this host". It's the standard way to listen for connections coming from
    anywhere, at least in examples that don't need IPv6 support. If we used
    `localhost` instead, it would work when the client and the server were on
    the same machine, but it would reject connections from the network.

[^writeln]: We could use `write!` or `writeln!` instead of `format!` to avoid
    allocating a `String` here, but that results in three write system calls,
    one for the prefix, one for number, and one more for the newline. That's
    probably slower than allocating. Separate writes would also appear as
    separate reads on the client side, and we'd need to do line buffering to
    avoid garbled output when we run multiple clients at once below. It's not
    guaranteed that the `format!` approach will come out as a single read on
    the client side, but in small examples like these it generally does.

```rust
LINK: Playground playground://async_playground/single_threaded_client.rs
fn main() -> io::Result<()> {
    let mut socket = TcpStream::connect("localhost:8000")?;
    io::copy(&mut socket, &mut io::stdout())?;
    Ok(())
}
```

This client opens a connection to the server and copies all the bytes that it
receives to standard output. It doesn't explicitly sleep, but it still takes a
second, because the server takes a second to finish responding.

We can run those examples locally, but they won't work as-is on the Playground,
because different Playground examples are isolated from each other. Let's work
around that by putting the client and the server together in the same program.
Since they're both blocking, we'll need to run them on different threads. We'll
rename their `main` functions to `server_main` and `client_main`, and while
we're at it, we'll run ten clients together at the same time:

```rust
LINK: Playground playground://async_playground/two_threaded_client_server.rs
fn main() -> io::Result<()> {
    // Open the listener first, to avoid racing against the server thread.
    let listener = TcpListener::bind("0.0.0.0:8000")?;
    // Start the server on a background thread.
    thread::spawn(|| server_main(listener).unwrap());
    // Run ten clients on ten different threads.
    let mut client_handles = Vec::new();
    for _ in 1..=10 {
        client_handles.push(thread::spawn(client_main));
    }
    for handle in client_handles {
        handle.join().unwrap()?;
    }
    Ok(())
}
```

This works, and we can run it on the Playground, but it takes ten seconds. Even
though the clients are running in parallel, the server is only responding to
one of them at a time. Let's make the server spawn a new thread for each
incoming request:[^background]

[^background]: This server loop will never `join` the threads it spawns, but we
    don't want to ignore any errors they might return. Unwrapping an error in a
    background thread will unwind the thread and print to standard error, which
    is better than nothing. Also, the `move` keyword is necessary here because
    otherwise that closure would borrow `n`, which violates the `'static`
    requirement of `thread::spawn`. Rust is right to complain about this,
    because if `server_main` returned while response threads were still
    running, pointers to `n` would become dangling.

```rust
LINK: Playground playground://async_playground/threads_client_server.rs
fn one_response(n: u64, mut socket: TcpStream) -> io::Result<()> {
    let start_msg = format!("start {n}\n");
    socket.write_all(start_msg.as_bytes())?;
    thread::sleep(Duration::from_secs(1));
    let end_msg = format!("end {n}\n");
    socket.write_all(end_msg.as_bytes())?;
    Ok(())
}

fn server_main(listener: TcpListener) -> io::Result<()> {
    let mut n = 1;
    loop {
        let (socket, _) = listener.accept()?;
        thread::spawn(move || one_response(n, socket).unwrap());
        n += 1;
    }
}
```

Great, it still works, and now it only takes one second. Threads are
convenient, at least when the compiler is happy with them. But as we saw in the
introduction, spawning a new thread for every request doesn't work well once
there are thousands of requests flying around. This is why we've gone through
all this trouble to learn async/await. So, how do we use async/await with
sockets?

## Poll

[TODO: poll-based IO example](playground://async_playground/client_server.rs)

---

<div class="prev-next-arrows">
    <div><a href="async_tasks.html">← Part Two: Tasks</a></div>
    <div class="space"> </div><div>
</div>
