# Async Rust, Part Three: IO
###### \[DRAFT]

- [Introduction](async_intro.html)
- [Part One: Futures](async_futures.html)
- [Part Two: Tasks](async_tasks.html)
- Part Three: IO (you are here)
  - [Nonblocking](#nonblocking)
  - [Poll](#poll)

Of course, async/await wasn't invented just for sleeping. The goal all along
has been efficient IO, especially network IO. Now that we understand futures
and tasks, we have all the tools we need to do some real work.

Let's start with a pair of ordinary, non-async examples. Here's a toy server
program:

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
    one for the prefix, one for the number, and one more for the newline.
    That's probably slower than allocating. Separate writes would also appear
    as separate reads on the client side, and we'd need to do line buffering to
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

This client opens a connection to the server and copies all the bytes it
receives to standard output. It doesn't explicitly sleep, but it still takes a
second, because the server takes a second to finish responding. Under the
covers, [`io::copy`] is a convenience wrapper around the standard
[`Read::read`] method on [`TcpStream`], which blocks until input arrives.

[`io::copy`]: https://doc.rust-lang.org/stable/std/io/fn.copy.html
[`Read::read`]: https://doc.rust-lang.org/stable/std/io/trait.Read.html#tymethod.read
[`TcpStream`]: https://doc.rust-lang.org/std/net/struct.TcpStream.html

We can run these examples locally, but they can't talk to each on the
Playground. Let's work around that by putting the client and the server
together in the same program. Since they're both blocking, we need to run them
on different threads. We'll rename their `main` functions to `server_main` and
`client_main`, and while we're at it, we'll run ten clients together at the
same time:[^unwrap]

[^unwrap]: Note that the return type of `join` in this example is a nested
    result, `thread::Result<io::Result<()>>`. IO errors from client threads
    wind up in the inner `Result` and are handled with `?`. The outer `Result`
    represents whether the client thread panicked, and we propagate those
    panics with `.unwrap()`. The server thread normally runs forever, so we
    can't `join` it. If it does short-circuit with an error, though, we don't
    want that error to be silent. Unwrapping server thread IO errors case
    prints to stderr in that case, which is better than nothing.

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
incoming request:[^move]

[^move]: The `move` keyword is necessary here because otherwise the closure
    would borrow `n`, which violates the `'static` requirement of
    `thread::spawn`. Rust is right to complain about this, because if
    `server_main` returned while response threads were still running, pointers
    to `n` would become dangling.

```rust
LINK: Playground playground://async_playground/threads_client_server.rs
HIGHLIGHT: 1, 7-17
fn one_response(mut socket: TcpStream, n: u64) -> io::Result<()> {
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
        thread::spawn(move || one_response(socket, n).unwrap());
        n += 1;
    }
}
```

Great, it still works, and now it only takes one second. Threads are
convenient, but as we saw in the introduction, spawning a new thread for every
request won't work when there are thousands of requests flying around. This is
why we've gone through all this trouble to learn async/await. So, how do we use
async/await with sockets?

## Nonblocking

There are two changes we need to make. First, we need to stop our reads from
blocking when no input is available. [`TcpListener`] and [`TcpStream`] both
support [`set_nonblocking`], which makes `accept` or `read` return
[`ErrorKind::WouldBlock`][error_kind] instead. Great! This change by itself is
technically enough for us to implement some async IO, if we didn't mind burning
100% CPU calling `accept` and `read` non-stop. But of course that's not what we
want, so the second thing we'll need is some way to sleep the main loop until
input arrives.

[`TcpListener`]: https://doc.rust-lang.org/std/net/struct.TcpListener.html
[`set_nonblocking`]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_nonblocking
[error_kind]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html

## Poll

We're going to use the [`poll`] system call for this, which is available on all
Unix-like OSs, including Linux and macOS.[^other_apis] We'll do it by calling a
C standard library function, [`libc::poll`]. 

[`poll`]: https://man7.org/linux/man-pages/man2/poll.2.html
[`libc::poll`]: https://docs.rs/libc/latest/libc/fn.poll.html

[^other_apis]: There are _many_ APIs we could use for this. The oldest is
    [`select`], which is similar to `poll` but kind of deprecated. More modern
    options include [`epoll`] and [`io_uring`] on Linux, [`kqueue`] on macOS
    and BSD, and IOCP on Windows. For a low-ish-level, cross-platform Rust
    library that abstracts over several of these, see [`mio`].

[`select`]: https://man7.org/linux/man-pages/man2/select.2.html
[`epoll`]: https://man7.org/linux/man-pages/man7/epoll.7.html
[`io_uring`]: https://man7.org/linux/man-pages/man7/io_uring.7.html
[`kqueue`]: https://man.freebsd.org/cgi/man.cgi?query=kqueue&sektion=2
[IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
[`mio`]: https://github.com/tokio-rs/mio

[TODO: poll-based IO example](playground://async_playground/client_server.rs)

---

<div class="prev-next-arrows">
    <div><a href="async_tasks.html">← Part Two: Tasks</a></div>
    <div class="space"> </div><div>
</div>
