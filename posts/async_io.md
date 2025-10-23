# Async Rust, Part Three: IO
###### 2024 October 23<sup>rd</sup>

- [Introduction](async_intro.html)
- [Part One: Futures](async_futures.html)
- [Part Two: Tasks](async_tasks.html)
- Part Three: IO (you are here)
  - [Threads](#threads)
  - [Non-blocking](#non_blocking)
  - [Poll](#poll)

Of course async/await isn't just for sleeping. Our goal all along has been IO,
especially network IO. Now that we have futures and tasks, we can start doing
some real work.

Let's go back to ordinary, non-async Rust for a moment. We'll start with a toy
server program and a client that talks to it. Then we'll use threads to combine
the server and several clients into one example that we can run on the
Playground. Once that combination is working, we'll translate it into async,
building on [the main loop we wrote in Part Two][part_two_impl].

Here's our toy server:

```rust
LINK: Playground ## playground://async_playground/single_threaded_server.rs
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

It starts listening on port 8000.[^zero_ip] For each connection it receives it
writes a start message, sleeps for one second, and writes an end
message.[^writeln] Here's a client for our toy server:

[^zero_ip]: `0.0.0.0` is the special IP address that means "all IPv4 interfaces
    on this host". It's the standard way to listen for connections coming from
    anywhere, at least in examples that don't need IPv6 support. If we bound
    the listener to `localhost` instead, it would work when the client and the
    server were on the same machine, but it would reject connections from the
    network.

[^writeln]: We could use `write!` or `writeln!` instead of `format!` to avoid
    allocating a `String` here, but that results in three separate writes to
    the `TcpStream`, one for the prefix, one for the number, and one more for
    the newline. That's probably slower than allocating. Separate writes also
    tend to appear as separate reads on the client side, so we'd need to do
    line buffering to avoid garbled output when we run multiple clients at once
    below. It's not guaranteed that the `format!` approach will come out as one
    read, but in small examples like these it generally does.

```rust
LINK: Playground ## playground://async_playground/single_threaded_client.rs
fn main() -> io::Result<()> {
    let mut socket = TcpStream::connect("localhost:8000")?;
    io::copy(&mut socket, &mut io::stdout())?;
    Ok(())
}
```

This client opens a connection to the server[^no_request] and copies all the
bytes it receives to standard output, as soon as they arrive. It doesn't
explicitly sleep, but it still takes a second, because the server takes a
second to finish responding. Under the covers, [`io::copy`] is a convenience
wrapper around the standard [`Read::read`] and [`Write::write`] methods, and
`read` blocks until input arrives.

[^no_request]: Our server starts sending response bytes as soon as our client
    connects to it, which makes this example as simple as possible. In real
    world protocols like HTTP, though, the client would need to send a request
    first.

[`io::copy`]: https://doc.rust-lang.org/stable/std/io/fn.copy.html
[`Read::read`]: https://doc.rust-lang.org/stable/std/io/trait.Read.html#tymethod.read
[`Write::write`]: https://doc.rust-lang.org/std/io/trait.Write.html#tymethod.write
[`TcpStream`]: https://doc.rust-lang.org/std/net/struct.TcpStream.html

These programs can't talk to each other on the Playground. You might want to
take the time to run them on your computer, or even better on two different
computers on your WiFi network.[^localhost] If you haven't done this before,
seeing it work on a real network is pretty cool. Reviewing the web server
project from [Chapter&nbsp;21 of The Book][ch21] might be helpful too.

[ch21]: https://doc.rust-lang.org/book/ch21-00-final-project-a-web-server.html
[part_two_impl]: playground://async_playground/tasks.rs

[^localhost]: In that case you'll need to change `localhost` in the client to
    the IP address of your server.

## Threads

Let's get this working on the Playground by putting the client and server
together in one program. Since they're both blocking, we'll have to run them on
separate threads. We'll rename their `main` functions to `client_main` and
`server_main`, and while we're at it we'll run ten clients at the same
time:[^unwrap]

[^unwrap]: Note that the return type of `handle.join()` in this example is
    `thread::Result<io::Result<()>>`, i.e. a `Result` nested in another
    `Result`. IO errors from client threads wind up in the inner `Result` and
    are handled with `?`. The outer `Result` represents whether the client
    thread panicked, and we propagate those panics with `.unwrap()`. The server
    thread normally runs forever, so we can't `join` it. If it does
    short-circuit with an error, though, we don't want that error to be silent.
    Unwrapping server thread IO errors prints to stderr in that case, which is
    better than nothing.

```rust
LINK: Playground ## playground://async_playground/two_threaded_client_server.rs
fn main() -> io::Result<()> {
    // Avoid a race between bind and connect by binding before spawn.
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

This works on the Playground, but it takes ten seconds. Even though the clients
are running in parallel, the server is only responding to one of them at a
time. Let's make the server spawn a new thread for each incoming
request:[^move]

[^move]: The `move` keyword is necessary here because otherwise the closure
    would borrow `n`, which violates the `'static` requirement of
    `thread::spawn`. Rust is right to complain about this, because if
    `server_main` returned while response threads were still running, pointers
    to `n` would become dangling.

```rust
LINK: Playground ## playground://async_playground/threads_client_server.rs
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

It still works, and now it only takes one second. This is exactly the behavior
we want. Now we're ready for our final project: expanding [the main loop from
Part Two][part_two_impl] and translating this example into async.

There are two big problems we need to solve. First, we need IO functions that
return immediately instead of blocking, even when there's no input yet, so that
we can use them in `Future::poll`.[^remember] Second, when all our tasks are
waiting for input, we want to sleep instead of busy looping, and we need a way
to wake up when any input arrives.

[^remember]: Remember that blocking in `poll` holds up the entire main loop,
    which in our single-threaded implementation will block _all_ tasks. That's
    always a performance issue, but in this case it's a correctness issue too.
    Once we get this example working, we'll have ten client tasks waiting to
    read input from the server task. If a client task blocks the server task,
    then input will never arrive, and the program will deadlock.

## Non-blocking

There's a solution for the first problem in the standard
library.[^three_quarters] [`TcpListener`] and [`TcpStream`] both have
[`set_nonblocking`] methods, which make `accept`, `read`, and `write` return
[`ErrorKind::WouldBlock`][error_kind] instead of blocking.

[^three_quarters]: Well, there's three quarters of a solution. For the rest
    we're gonna cheat&hellip;

[`TcpListener`]: https://doc.rust-lang.org/std/net/struct.TcpListener.html
[`set_nonblocking`]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_nonblocking
[error_kind]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html

Technically, `set_nonblocking` by itself is enough to get async IO working.
Without solving the second problem, we'll burn 100% CPU busy looping until we
exit, but our output will still be correct, and we can lay a lot of groundwork
before we get to the more complicated part.

When we wrote `Foo`, `JoinAll`, and `Sleep` in Part One, each of them required
a struct definition, a `poll` function, and a constructor function. To cut down
on boilerplate this time around, we'll use [`std::future::poll_fn`], which
takes a standalone `poll` function and generates the rest of the future.

[`std::future::poll_fn`]: https://doc.rust-lang.org/stable/std/future/fn.poll_fn.html

There are four potentially blocking operations that we need to async-ify.
There's `accept` and `write` on the server side, and there's `connect` and
`read` on the client side. Let's start with `accept`:[^async_wrapper]

[^async_wrapper]: We're writing this as an async function that creates a future
    and then immediately awaits it, but we could also have written it as a
    non-async function that returns that future. That would be cleaner, but
    we'd need lifetimes in the function signature, and [the "obvious" way to
    write them turns out to be subtly incorrect][outlives_trick]. The 2024
    Edition will fix this by changing the way that "return position `impl
    Trait`" types "capture" lifetime parameters.

[outlives_trick]: https://rust-lang.github.io/rfcs/3498-lifetime-capture-rules-2024.html#the-outlives-trick

[client_server_busy_orig]: playground://async_playground/client_server_busy.rs

```rust
LINK: Playground ## https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=69f9bc4f6964a57bd92e24c2287e9636
async fn accept(
    listener: &mut TcpListener,
) -> io::Result<(TcpStream, SocketAddr)> {
    std::future::poll_fn(|context| match listener.accept() {
        Ok((stream, addr)) => {
            stream.set_nonblocking(true)?;
            Poll::Ready(Ok((stream, addr)))
        }
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
            // TODO: This is a busy loop.
            context.waker().wake_by_ref();
            Poll::Pending
        }
        Err(e) => Poll::Ready(Err(e)),
    }).await
}
```

The key here is handling `WouldBlock` errors by converting them to `Pending`.
Calling `wake_by_ref` whenever we return `Pending`, like we did in [the second
version of `Sleep` from Part One][sleep_busy], makes this a busy loop. We'll
fix that in the next section. We're assuming that the `TcpListener` is already
in non-blocking mode,[^eintr] and we're putting the returned `TcpStream` into
non-blocking mode too,[^io_result] to get ready for async writes.

[sleep_busy]: playground://async_playground/sleep_busy.rs

Next let's implement those writes. If we wanted to copy Tokio, we'd define an
[`AsyncWrite`] trait and make everything generic, but that's a lot of code.
Instead, let's keep it short and hardcode that we're writing to a `TcpStream`:

[^eintr]: And we're going to [assume that non-blocking calls never return
    `ErrorKind::Interrupted`/`EINTR`][eintr], so we don't need an extra line of
    code in each example to retry that case.

[eintr]: https://stackoverflow.com/a/14485305/823869

[^io_result]: Eagle-eyed readers might spot that our `poll_fn` closure is using
    the `?` operator with `set_nonblocking`, even though the closure itself
    returns `Poll`. This works because there's [a `Try` implementation for
    `Poll<Result<…>>`][try_poll_result] that uses the same associated
    `Residual` type as [the `Try` implementation for `Result<…>`][try_result].
    See [RFC 3058] for the details of the `Try` trait, which are still unstable
    as of Rust&nbsp;1.82.

[try_poll_result]: https://doc.rust-lang.org/stable/std/ops/trait.Try.html#impl-Try-for-Poll%3CResult%3CT,+E%3E%3E
[try_result]: https://doc.rust-lang.org/stable/std/ops/trait.Try.html#impl-Try-for-Result%3CT,+E%3E
[RFC 3058]: https://rust-lang.github.io/rfcs/3058-try-trait-v2.html

```rust
LINK: Playground ## https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=69f9bc4f6964a57bd92e24c2287e9636
async fn write_all(
    mut buf: &[u8],
    stream: &mut TcpStream,
) -> io::Result<()> {
    std::future::poll_fn(|context| {
        while !buf.is_empty() {
            match stream.write(&buf) {
                Ok(0) => {
                    let e = io::Error::from(io::ErrorKind::WriteZero);
                    return Poll::Ready(Err(e));
                }
                Ok(n) => buf = &buf[n..],
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // TODO: This is a busy loop.
                    context.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }
        Poll::Ready(Ok(()))
    }).await
}
```

`TcpStream::write` isn't guaranteed to consume all of `buf`, so we need to call
it in a loop, bumping `buf` forward each time. It's unlikely that we'll see
`Ok(0)` from `TcpStream`,[^ok_0] but if we do it's better for that to be an
error than an infinite loop. The loop condition also means that we won't make
any calls to `write` if `buf` is initially empty, which matches the default
behavior of [`Write::write_all`].[^write_all]

[^ok_0]: `Ok(0)` from a write means that either the input `buf` was empty,
    which is ruled out by our `while` condition, or that the writer can't
    accept more bytes. The latter mostly applies to not-real-IO writers like
    [`&mut [u8]`][mut_u8_writer]. When real-IO writers like `TcpStream` or
    `File` can't accept more bytes (because the other end is closed or the disk
    is full) they usually indicate that with `Err` rather than `Ok(0)`.

[mut_u8_writer]: https://doc.rust-lang.org/std/io/trait.Write.html#impl-Write-for-%26mut+%5Bu8%5D
[`Write::write_all`]: https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all

[^write_all]: It would be nice to use `Write::write_all` directly here and get
    the loop and the `WriteZero` handling for free. But unfortunately, when
    `Write::write_all` returns `WouldBlock`, it doesn't tell us how many bytes
    it wrote before that, and we need that number to update `buf`. In contrast,
    if `Write::write` needs to block after it's already written some bytes, it
    returns `Ok(n)` first, and then the _next_ call returns `WouldBlock`.

[`Write::write_all`]: https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all

Those are the async building blocks we needed for the server, and now we can
write the async version of `server_main`:[^rely_on_pin]

[^rely_on_pin]: I'm pretty sure this is the first time we've implicitly relied
    on `Pin` guarantees for soundness. The compiler-generated `one_response`
    future owns a `TcpStream`, but it also passes references to that stream
    into `write_all` futures, and it owns those too. That would be unsound if
    the `one_response` future could move (thus moving the `TcpStream`) after
    those borrows were established.

```rust
LINK: Playground ## https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=69f9bc4f6964a57bd92e24c2287e9636
HIGHLIGHT: 1, 3-4, 6, 10, 13-14
async fn one_response(mut socket: TcpStream, n: u64) -> io::Result<()> {
    let start_msg = format!("start {n}\n");
    write_all(start_msg.as_bytes(), &mut socket).await?;
    sleep(Duration::from_secs(1)).await;
    let end_msg = format!("end {n}\n");
    write_all(end_msg.as_bytes(), &mut socket).await?;
    Ok(())
}

async fn server_main(mut listener: TcpListener) -> io::Result<()> {
    let mut n = 1;
    loop {
        let (socket, _) = accept(&mut listener).await?;
        spawn(async move { one_response(socket, n).await.unwrap() });
        n += 1;
    }
}
```

Similar to [the threads example we started with](#threads), we never join
server tasks, so we use `unwrap` to print to stderr if they fail.[^take_down]
Previously we did that inside a closure, and here we do it inside an `async`
block, which works like an anonymous `async fn` that takes no arguments.

[^take_down]: In our case panicking in any task will print and then take down
    the whole process, because we're not using background threads, and we're
    not [catching panics]. But as we noted with `JoinHandle` in Part Two, Tokio
    does catch panics, even in [single-threaded mode].

[catching panics]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
[single-threaded mode]: https://docs.rs/tokio/latest/tokio/attr.main.html#current-thread-runtime

Hopefully that works, but we need to translate the client before we can test
it.

We just did async writes, so let's do async reads. The counterpart of
`Write::write_all` is [`Read::read_to_end`], but that's not quite what we want
here. We want to print output as soon as it arrives, rather than collecting it
in a `Vec` and printing it all at the end. Let's keep things short again and
hardcode the printing. We'll call it `print_all`:[^copy]

[`Read::read_to_end`]: https://doc.rust-lang.org/std/io/trait.Read.html#method.read_to_end

[^copy]: In Tokio we'd use [`tokio::io::copy`] for this, the same way we used
    [`std::io::copy`] in the non-async client. Writing a generic, async `copy`
    function would mean we'd need [`AsyncRead`] and [`AsyncWrite`] traits and
    implementations too, though, and that's a lot more code.

[`tokio::io::copy`]: https://docs.rs/tokio/latest/tokio/io/fn.copy.html
[`std::io::copy`]: https://doc.rust-lang.org/std/io/fn.copy.html
[`AsyncRead`]: https://docs.rs/tokio/latest/tokio/io/trait.AsyncRead.html
[`AsyncWrite`]: https://docs.rs/tokio/latest/tokio/io/trait.AsyncWrite.html

```rust
LINK: Playground ## https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=69f9bc4f6964a57bd92e24c2287e9636
async fn print_all(stream: &mut TcpStream) -> io::Result<()> {
    std::future::poll_fn(|context| {
        loop {
            let mut buf = [0; 1024];
            match stream.read(&mut buf) {
                Ok(0) => return Poll::Ready(Ok(())), // EOF
                // Assume that printing doesn't block.
                Ok(n) => io::stdout().write_all(&buf[..n])?,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // TODO: This is a busy loop.
                    context.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }
    }).await
}
```

`Ok(0)` from a read means end-of-file, but otherwise this is similar to
`write_all` above.[^println]

[^println]: We're cheating a little bit by assuming that printing doesn't
    block, but that's not really any different from using `println!` in an
    async function, which we've been doing the whole time. Realistically,
    programs that write enough to stdout to fill their pipe buffer (`tar` or
    `gzip` for example) can't make progress when their output is blocked
    anyway, and it's common to ignore this.

The other async building block we need for our client is `connect`, but there
are a couple of problems with that. First, `TcpStream::connect` creates a new
stream, and there's no way for us to call `set_nonblocking` on that stream
before `connect` talks to the network.[^socket2] Second, `connect` can include
a DNS lookup, and async DNS is a whole can of worms.[^dns] Solving those
problems here would be a lot of trouble without much benefit&hellip;so we're
going to cheat and just assume that `connect` doesn't block.[^huge_cheat]

[^socket2]: We would need to solve this with the [`socket2`] crate, which
    separates [`Socket::new`] from [`Socket::connect`].

[`socket2`]: https://docs.rs/socket2
[`Socket::new`]: https://docs.rs/socket2/latest/socket2/struct.Socket.html#method.new
[`Socket::connect`]: https://docs.rs/socket2/latest/socket2/struct.Socket.html#method.connect

[^dns]: DNS needs to read config files like `/etc/resolv.conf`, so the OS
    implementation is in libc rather than in the kernel, and libc only exposes
    blocking interfaces like [`getaddrinfo`]. Those configs are unstandardized
    and platform-specific, and reading them is a pain. Even Tokio punts on this
    and [makes a blocking call to `getaddrinfo` on a thread pool][tokio_dns].
    For comparison, the `net` module in the Golang standard library [contains
    two DNS implementations][golang_fallback], an async resolver for simple
    cases, and a fallback resolver that also calls `getaddrinfo` on a thread
    pool.

[`getaddrinfo`]: https://man7.org/linux/man-pages/man3/getaddrinfo.3.html
[tokio_dns]: https://github.com/tokio-rs/tokio/blob/tokio-1.40.0/tokio/src/net/addr.rs#L182-L184
[golang_fallback]: https://pkg.go.dev/net#hdr-Name_Resolution

[^huge_cheat]: This is big-time cheating, but our example only connects to
    itself, so we'll get away with it. In the real world we'd use a proper
    async implementation like [`tokio::net::TcpStream::connect`].

[`tokio::net::TcpStream::connect`]: https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html#method.connect

With one real async building block and one blatant lie, we can write
`client_main`:

```rust
LINK: Playground ## https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=69f9bc4f6964a57bd92e24c2287e9636
async fn client_main() -> io::Result<()> {
    // XXX: Assume that connect() returns quickly.
    let mut socket = TcpStream::connect("localhost:8000")?;
    socket.set_nonblocking(true)?;
    print_all(&mut socket).await?;
    Ok(())
}
```

And finally `async_main`:

```rust
LINK: Playground ## https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=69f9bc4f6964a57bd92e24c2287e9636
async fn async_main() -> io::Result<()> {
    // Avoid a race between bind and connect by binding before spawn.
    let listener = TcpListener::bind("0.0.0.0:8000")?;
    listener.set_nonblocking(true)?;
    // Start the server on a background task.
    spawn(async { server_main(listener).await.unwrap() });
    // Run ten clients as ten different tasks.
    let mut task_handles = Vec::new();
    for _ in 1..=10 {
        task_handles.push(spawn(client_main()));
    }
    for handle in task_handles {
        handle.await?;
    }
    Ok(())
}
```

It works! It busy loops and burns 100% CPU, but it really does work. That's a
lot of groundwork laid.

## Poll

The second big problem we need to solve is sleeping the main loop until input
arrives. This isn't something we can do on our own, and we need help from the
OS. We're going to use the [`poll`] "system call" for this,[^name] which is
available on all Unix-like OSs, including Linux and macOS.[^syscall] We'll
invoke it using the C standard library function [`libc::poll`], which looks
like this in Rust:

[`poll`]: https://man7.org/linux/man-pages/man2/poll.2.html

[^name]: It's no coincidence that Rust's `Future::poll` interface shares its
    name with the `poll` system call and the C standard library function that
    wraps it. They solve different layers of the same problem, managing many IO
    operations at the same time without a busy loop.

[^syscall]: We use "syscalls" all the time under the covers, but we don't often
    call them directly. Basic OS features like files and threads work roughly
    the same way across common OSs, so standard library abstractions like
    `File` and `Thread` are usually all we need. But async IO is a different
    story: The interfaces provided by different OSs vary widely, and the world
    hasn't yet settled on one right way to do it. We'll use [`poll`] in these
    examples because it's relatively simple and widely supported, but there are
    many other options. The oldest is [`select`], which is similar to `poll`
    but kind of deprecated. Modern, higher-performance options include
    [`epoll`] and [`io_uring`] on Linux, [`kqueue`] on macOS and BSD, and
    [IOCP] on Windows. For a medium-level, cross-platform Rust library that
    abstracts over several of these, see [`mio`].

[`select`]: https://man7.org/linux/man-pages/man2/select.2.html
[`epoll`]: https://man7.org/linux/man-pages/man7/epoll.7.html
[`io_uring`]: https://man7.org/linux/man-pages/man7/io_uring.7.html
[`kqueue`]: https://man.freebsd.org/cgi/man.cgi?query=kqueue&sektion=2
[IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
[`mio`]: https://github.com/tokio-rs/mio
[`libc::poll`]: https://docs.rs/libc/latest/libc/fn.poll.html

```rust
pub unsafe extern "C" fn poll(
    fds: *mut pollfd,
    nfds: nfds_t,
    timeout: c_int,
) -> c_int
```

`libc::poll` takes a list[^c_style] of "poll file descriptors" and a timeout in
milliseconds. The timeout will let us wake up for sleeps in addition to IO,
replacing `thread::sleep` in our main loop. Each [`pollfd`] looks like this:

[^c_style]: As usual with C functions, the list is split into two arguments, a
    raw pointer to the first element and a count of elements.

[`pollfd`]: https://docs.rs/libc/latest/libc/struct.pollfd.html

```rust
struct pollfd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}
```

The `fd` field is a "file descriptor", or in Rust terms a "raw" file
descriptor. It's an identifier that Unix-like OSs use to track open resources
like files and sockets. We can get a descriptor from a `TcpListener` or a
`TcpStream` by calling [`.as_raw_fd()`][as_raw_fd], which returns a [`RawFd`],
a type alias for `c_int`.[^windows]

[as_raw_fd]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.as_raw_fd
[`RawFd`]: https://doc.rust-lang.org/std/os/fd/type.RawFd.html

[^windows]: Apart from `poll` not existing on Windows, none of these raw file
    descriptor operations will compile on Windows either. The Windows
    counterpart of `as_raw_fd` is [`as_raw_handle`]. This is a low enough level
    of detail that the Rust standard library doesn't try to abstract over
    platform differences.

[`as_raw_handle`]: https://doc.rust-lang.org/std/os/windows/io/trait.AsRawHandle.html

The `events` field is a collection of bitflags indicating what we're waiting
for. The most common events are [`POLLIN`], meaning input is available, and
[`POLLOUT`], meaning space is available in output buffers. We'll wait for
`POLLIN` when we get `WouldBlock` from a read, and we'll wait for `POLLOUT`
when we get `WouldBlock` from a write.

[`POLLIN`]: https://docs.rs/libc/latest/libc/constant.POLLIN.html
[`POLLOUT`]: https://docs.rs/libc/latest/libc/constant.POLLOUT.html

The `revents` field ("returned events") is similar but used for output rather
than input. After `poll` returns, the bits in this field indicate whether the
corresponding descriptor was one of the ones that caused the wakeup. We could
use this to poll only the specific tasks that the wakeup is for, but for
simplicity we'll ignore this field and poll every task every time we wake up.

Our async IO functions, `accept`, `write_all`, and `print_all`, need a way to
send `pollfd`s and `Waker`s back to `main`, so that `main` can call
`libc::poll`. We'll add a couple more global `Vec`s for this, plus a helper
function to populate them:[^lock_order]

[^lock_order]: Whenever we hold more than one lock at a time, we need to make
    sure that all callers lock them in the same order, to avoid deadlocks.
    We're locking `POLL_FDS` before `POLL_WAKERS` here, so we'll do the same in
    `main`.

[client_server_poll_orig]: playground://async_playground/client_server_poll.rs

```rust
LINK: Playground ## https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=50a7e9dd92e4b6290e87d8f9434fff71
static POLL_FDS: Mutex<Vec<libc::pollfd>> = Mutex::new(Vec::new());
static POLL_WAKERS: Mutex<Vec<Waker>> = Mutex::new(Vec::new());

fn register_pollfd(
    context: &mut Context,
    fd: &impl AsRawFd,
    events: libc::c_short,
) {
    let mut poll_fds = POLL_FDS.lock().unwrap();
    let mut poll_wakers = POLL_WAKERS.lock().unwrap();
    poll_fds.push(libc::pollfd {
        fd: fd.as_raw_fd(),
        events,
        revents: 0,
    });
    poll_wakers.push(context.waker().clone());
}
```

Now our async IO functions can call `register_pollfd` instead of `wake_by_ref`.
`accept` and `print_all` are reads, so they handle `WouldBlock` by setting
`POLLIN`:

```rust
LINK: Playground ## https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=50a7e9dd92e4b6290e87d8f9434fff71
HIGHLIGHT: 2
Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    register_pollfd(context, listener, libc::POLLIN);
    Poll::Pending
}
```

`write_all` handles `WouldBlock` by setting `POLLOUT`:

```rust
LINK: Playground ## https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=50a7e9dd92e4b6290e87d8f9434fff71
HIGHLIGHT: 2
Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    register_pollfd(context, stream, libc::POLLOUT);
    return Poll::Pending;
}
```

Finally, `main`. We'll start by preparing the `timeout` argument for
`libc::poll`. This is similar to how we've been computing the next wake time
all along, except now we're not guaranteed to have one,[^no_wake] and we need
to convert it to milliseconds:

[^no_wake]: Previously, sleeping forever could only be a bug, but now that we
    can wait on IO in addition to sleeping, waiting forever is valid.

```rust
LINK: Playground ## https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=50a7e9dd92e4b6290e87d8f9434fff71
HIGHLIGHT: 6-15
// Some tasks might wake other tasks. Re-poll if the AwakeFlag has been
// set. Polling futures that aren't ready yet is inefficient but allowed.
if awake_flag.check_and_clear() {
    continue;
}
// All tasks are either sleeping or blocked on IO. Use libc::poll to wait
// for IO on any of the POLL_FDS. If there are any WAKE_TIMES, use the
// earliest as a timeout.
let mut wake_times = WAKE_TIMES.lock().unwrap();
let timeout_ms = if let Some(time) = wake_times.keys().next() {
    let duration = time.saturating_duration_since(Instant::now());
    duration.as_millis() as libc::c_int
} else {
    -1 // infinite timeout
};
```

After all that preparation, we can replace `thread::sleep` with `libc::poll` in
the main loop. It's a "foreign" function, so calling it is `unsafe`:[^fd_ub]

[^fd_ub]: We know that the raw pointer is valid, and that `libc::poll` won't
    retain that pointer after returning. We might also worry about what happens
    if one of the descriptors in `POLL_FDS` came from a socket that's since
    been closed. In that case the descriptor might refer to nothing, or it
    might've been reused by the kernel to refer to an unrelated file or socket.
    Since `libc::poll` doesn't modify any of its arguments (including for
    example reading from a file, which would advance the cursor), the worst
    that can happen here is a "spurious wakeup", where some event for an
    unrelated file wakes us up early. Our code already handles busy loop
    polling, so spurious wakeups are no problem.

```rust
LINK: Playground ## https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=50a7e9dd92e4b6290e87d8f9434fff71
let mut poll_fds = POLL_FDS.lock().unwrap();
let mut poll_wakers = POLL_WAKERS.lock().unwrap();
let poll_error_code = unsafe {
    libc::poll(
        poll_fds.as_mut_ptr(),
        poll_fds.len() as libc::nfds_t,
        timeout_ms,
    )
};
if poll_error_code < 0 {
    return Err(io::Error::last_os_error());
}
```

Last of all, when we wake up and `libc::poll` returns, we need to clear
`POLL_FDS` and invoke all the `POLL_WAKERS`. The main loop still polls every
task every time, and tasks that aren't `Ready` will re-register themselves in
`POLL_FDS` before the next sleep.

```rust
LINK: Playground ## https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=50a7e9dd92e4b6290e87d8f9434fff71
HIGHLIGHT: 1-2
poll_fds.clear();
poll_wakers.drain(..).for_each(Waker::wake);
// Invoke Wakers from WAKE_TIMES if their time has come.
while let Some(entry) = wake_times.first_entry() {
    …
```

It works![^threads]

[^threads]: Similar to the end of Part Two, our implementation is technically
    thread-safe, but we don't yet have a way to wake up the main thread (i.e.
    force `libc::poll` to return) if a background thread invokes a `Waker` or
    spawns a task. The classic approach on Unix is to create an [`O_NONBLOCK`
    pipe] whose read end is always included in `POLL_FDS`, and then any thread
    can trigger a wakeup by writing a byte to that pipe. A more modern,
    Linux-specific option for this is an [`eventfd`]. If you've made it this
    far with energy to spare, getting one of those approaches working is a good
    final exercise.

[`O_NONBLOCK` pipe]: https://man7.org/linux/man-pages/man2/pipe.2.html
[`eventfd`]: https://man7.org/linux/man-pages/man2/eventfd.2.html

And that's it. We did it. Our main loop is finally an _event loop_.

Hopefully this little adventure has made async Rust and async IO in general
seem less magical. There's lots more to explore and look forward to, like
[future language features][future_features] and [all the gory details of
`Pin`][pin]. Good luck out there :)

[future_features]: https://smallcultfollowing.com/babysteps/blog/2024/01/03/async-rust-2024/
[pin]: https://without.boats/blog/pin/

---

<div class="prev-next-arrows">
    <div><a href="async_tasks.html">← Part Two: Tasks</a></div>
    <div class="space"> </div>
</div>
