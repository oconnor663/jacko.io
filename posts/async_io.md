# Async Rust, Part Three: IO
###### \[DRAFT]

- [Introduction](async_intro.html)
- [Part One: Futures](async_futures.html)
- [Part Two: Tasks](async_tasks.html)
- Part Three: IO (you are here)
  - [Threads](#threads)
  - [Non-blocking](#non_blocking)
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
    allocating a `String` here, but that results in three writes to the
    `TcpStream`, one for the prefix, one for the number, and one more for the
    newline. That's probably slower than allocating. Separate writes also tend
    to appear as separate reads on the client side, so we'd need to do line
    buffering to avoid garbled output when running multiple clients at once.
    It's not guaranteed that the `format!` approach will come out as one read,
    but in small examples like these it generally does.

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

## Threads

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

It still works, and now it only takes one second. This is exactly the behavior
we want, but using async IO we can run it on one thread instead of eleven.
We'll expand [our async/await implementation from Part Two][part_two_impl].

[part_two_impl]: playground://async_playground/tasks.rs

There are two problems we need to solve. First, we need [`TcpListener::accept`]
and [`TcpStream::read`] to return immediately instead of blocking, even when
there's no input yet, so that we can call them in `poll`.[^remember] Second,
when all tasks are pending, we want to sleep instead of busy looping, and we
need a way to wake up when any input arrives.

[`TcpListener::accept`]: https://doc.rust-lang.org/std/net/struct.TcpListener.html#method.accept
[`TcpStream::read`]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.read

[^remember]: Remember that blocking in `poll` holds up the entire main loop,
    which in our single-threaded implementation blocks _all_ tasks. That's a
    performance issue, but it's also a correctness issue. Once we get this
    example working, we'll have ten client tasks waiting to read input from a
    server task. If anything blocks the server task, that input will never
    arrive, and the program will deadlock.

## Non-blocking

The first problem is easier, because Rust has a solution in the standard
library. [`TcpListener`] and [`TcpStream`] both have a [`set_nonblocking`]
method, which makes `accept` and `read` return
[`ErrorKind::WouldBlock`][error_kind] instead of blocking.

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

There are four potentially blocking operations that we need async versions of.
There's `accept` and `write` on the server side, and there's `connect` and
`read` on the client side. Let's start with accept:[^async_wrapper]

[^async_wrapper]: We're writing this as an async function that creates a future
    and then immediately awaits it, but we could also have written it as a
    non-async function that returns that future. That would be cleaner, but
    we'd need lifetimes in the function signature, and [the "obvious" way to
    write them turns out to be subtly incorrect][outlives_trick]. Rust 2024
    Edition will fix this by changing the way that "return position `impl
    Trait`" types "capture" lifetime parameters.

[outlives_trick]: https://rust-lang.github.io/rfcs/3498-lifetime-capture-rules-2024.html#the-outlives-trick

```rust
LINK: Playground playground://async_playground/client_server_busy.rs
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

Calling `wake_by_ref` whenever we return `Pending`, like we did in [the second
version of `Sleep` from Part One][sleep_busy], makes this a busy loop. We'll
fix that in the next section. We're assuming that the `TcpListener` is already
in non-blocking mode, and we're putting the returned `TcpStream` into
non-blocking mode too, to get ready for async writes.[^io_result] Now let's
implement the writes:

[^io_result]: Eagle-eyed readers might spot that our `poll_fn` closure is using
    the `?` operator with `set_nonblocking`, even though the closure itself
    returns `Poll`. This works because there's [a `Try` implementation for
    `Poll<Result<...>>`][try_poll_result] that uses the same associated
    `Residual` type as [the `Try` implementation for
    `Result<...>`][try_result]. See [RFC 3058] for the details of the `Try`
    trait, which are still unstable as of Rust&nbsp;1.81.

[try_poll_result]: https://doc.rust-lang.org/stable/std/ops/trait.Try.html#impl-Try-for-Poll%3CResult%3CT,+E%3E%3E
[try_result]: https://doc.rust-lang.org/stable/std/ops/trait.Try.html#impl-Try-for-Result%3CT,+E%3E
[RFC 3058]: https://rust-lang.github.io/rfcs/3058-try-trait-v2.html

```rust
LINK: Playground playground://async_playground/client_server_busy.rs
async fn write_all(buf: &[u8], stream: &mut TcpStream) -> io::Result<()> {
    std::future::poll_fn(|context| {
        let mut position = 0;
        while position < buf.len() {
            match stream.write(&buf[position..]) {
                Ok(n) if n == 0 => {
                    let e = io::Error::from(io::ErrorKind::WriteZero);
                    return Poll::Ready(Err(e));
                }
                Ok(n) => position += n,
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

TODO[^dns]

[sleep_busy]: playground://async_playground/sleep_busy.rs

[^dns]: The `XXX` comment here marks the biggest shortcut we're going to take
    in these examples: assuming that [`TcpStream::connect`] doesn't block.
    We'll get away with that because we're just one process connecting to
    ourselves, but in the real world `connect` would make one or more DNS
    requests and then do a TCP handshake, and all of that is blocking.
    Non-blocking DNS is surprisingly difficult, because the implementation
    needs to read config files like `/etc/resolv.conf`, which means it's in
    libc rather than in the kernel, and libc only exposes blocking interfaces
    like [`getaddrinfo`]. Those configs are unstandardized and
    platform-specific, so implementing them is a pain, and even Tokio punts on
    this and [makes a blocking call to `getaddrinfo` on a thread
    pool][tokio_dns]. For comparison, the `net` module in the Golang standard
    library [contains two DNS implementations][golang_fallback], an async
    resolver for simple cases, and a fallback resolver that also calls
    `getaddrinfo` on a thread pool. That said, if you're connecting directly to
    an IP address and you don't need to do DNS, you can do a non-blocking
    `connect` using the [`socket2`] crate.

[golang_fallback]: https://pkg.go.dev/net#hdr-Name_Resolution
[`socket2`]: https://docs.rs/socket2

```rust
LINK: Playground playground://async_playground/client_server_busy.rs
async fn tcp_bind(address: &str) -> io::Result<TcpListener> {
    let listener = TcpListener::bind(address)?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

async fn tcp_connect(address: &str) -> io::Result<TcpStream> {
    // XXX: This is technically blocking. Assume it returns quickly.
    let socket = TcpStream::connect(address)?;
    socket.set_nonblocking(true)?;
    Ok(socket)
}
```

Next, the async version of [`TcpListener::accept`]:

```rust
LINK: Playground playground://async_playground/client_server_busy.rs
struct TcpAccept<'a> {
    listener: &'a TcpListener,
}

impl<'a> Future for TcpAccept<'a> {
    type Output = io::Result<TcpStream>;

    fn poll(
        self: Pin<&mut Self>,
        context: &mut Context,
    ) -> Poll<io::Result<TcpStream>> {
        match self.listener.accept() {
            Ok((stream, _)) => {
                let result = stream.set_nonblocking(true);
                Poll::Ready(result.and(Ok(stream)))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // TODO: This causes a busy loop.
                context.waker().wake_by_ref();
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

fn tcp_accept(listener: &TcpListener) -> TcpAccept {
    TcpAccept { listener }
}
```

This is enough for us to write `server_main`:

```rust
LINK: Playground playground://async_playground/client_server_busy.rs
async fn one_response(mut socket: TcpStream, n: u64) -> io::Result<()> {
    let start_msg = format!("start {n}\n");
    socket.write_all(start_msg.as_bytes())?;
    sleep(Duration::from_secs(1)).await;
    let end_msg = format!("end {n}\n");
    socket.write_all(end_msg.as_bytes())?;
    Ok(())
}

async fn server_main(listener: TcpListener) -> io::Result<()> {
    let mut n = 1;
    loop {
        let socket = tcp_accept(&listener).await?;
        spawn(async move { one_response(socket, n).await.unwrap() });
        n += 1;
    }
}
```

And an async version of [`std::io::copy`]:[^unpin]

[`std::io::copy`]: https://doc.rust-lang.org/stable/std/io/fn.copy.html

[^unpin]: Two comments about the `Copy` struct. First, it might've been
    more flexible to hold `R` and `W` by value instead of by mutable
    reference. `Read` and `Write` have ["blanket implementations"][blanket]
    for mutable references to any `Read` or `Write` type, so callers using
    references would still work. But if we did that, then `Copy` wouldn't
    automatically be `Unpin`, and we'd need to `impl Unpin for Copy` to
    access its fields. (Alternatively we could require `R` and `W` to be
    `Unpin`, but that would be unnecessarily restrictive for callers.)
    There's nothing wrong with doing that, and it's perfectly safe, but
    we're trying to avoid talking about `Unpin`. Shared and mutable
    references to any type are always `Unpin`, so holding references
    internally makes `Copy` automatically `Unpin` and lets us dodge this
    topic.
    <br>
    &emsp;&emsp;Second, note that we explicitly destructure references to
    `reader` and `writer` from a single call to `self.as_mut()`. With
    regular mutable references we can ["split borrows"][split] without any
    extra steps, but `Pin::as_mut` gets in the way of that. Similarly, the
    automatic ["reborrowing"][reborrowing] we get when we use mutable
    references as function arguments doesn't work with pinned references.
    It's possible that a future version of Rust might fix these papercuts
    by making ["pinned places"][pinned_places] a language feature on par
    with shared and mutable references.

[blanket]: https://doc.rust-lang.org/std/io/trait.Read.html#impl-Read-for-%26mut+R
[split]: https://doc.rust-lang.org/nomicon/borrow-splitting.html
[pinned_places]: https://without.boats/blog/pinned-places/
[reborrowing]: https://github.com/rust-lang/reference/issues/788

```rust
LINK: Playground playground://async_playground/client_server_busy.rs
struct Copy<'a, R, W> {
    reader: &'a mut R,
    writer: &'a mut W,
}

impl<'a, R: Read, W: Write> Future for Copy<'a, R, W> {
    type Output = io::Result<()>;

    fn poll(
        mut self: Pin<&mut Self>,
        context: &mut Context,
    ) -> Poll<io::Result<()>> {
        let Copy { reader, writer } = &mut *self.as_mut();
        match io::copy(reader, writer) {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // TODO: This causes a busy loop.
                context.waker().wake_by_ref();
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

fn copy<'a, R, W>(reader: &'a mut R, writer: &'a mut W) -> Copy<'a, R, W> {
    Copy { reader, writer }
}
```

Now we can write `client_main` and `async_main`:

```rust
LINK: Playground playground://async_playground/client_server_busy.rs
async fn client_main() -> io::Result<()> {
    let mut socket = tcp_connect("localhost:8000").await?;
    copy(&mut socket, &mut io::stdout()).await?;
    Ok(())
}

async fn async_main() -> io::Result<()> {
    // Open the listener first, to avoid racing against the server thread.
    let listener = tcp_bind("0.0.0.0:8000").await?;
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

It works!

## Poll

The second thing we need is a way for our main loop to sleep until input
arrives. We're going to use the [`poll`] "system call" for this, which is
available on all Unix-like OSs, including Linux and macOS.[^syscall] We'll call
it using the C standard library function [`libc::poll`].[^name] This function
takes a list of "poll file descriptors" and a timeout. The timeout will let us
wake up for sleeps in addition to IO, replacing `thread::sleep` in our main
loop. Each poll file descriptor looks like this:

[`poll`]: https://man7.org/linux/man-pages/man2/poll.2.html

[^syscall]: We use "syscalls" all the time under the covers, but we don't often
    call them directly. Basic OS features like files and threads work roughly
    the same way across common OSs, so standard library abstractions like
    `File` and `Thread` are usually all we need. But async IO is a different
    story: The interfaces provided by different OSs vary widely, and the world
    hasn't yet settled on a "right way to do it". We'll use [`poll`] in these
    examples because it's simpler and relatively widely supported, but there
    are many other options. The oldest is [`select`], which is similar to
    `poll` but kind of deprecated. Modern, higher-performance options include
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

[^name]: It's no coincidence that Rust's `Future::poll` interface shares its
    name with the `poll` system call and the C standard library function that
    wraps it. They solve different layers of the same problem, managing many IO
    operations at the same time without a busy loop.

```rust
struct pollfd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}
```

That `fd` field is a "file descriptor", or what Rust calls a "raw" file
descriptor. It's an identifier that Unix-like OSs use to track open resources
like files and sockets. We can get the descriptor from a `TcpListener` or a
`TcpStream` by calling [`.as_raw_fd()`][as_raw_fd], which returns [`RawFd`], a
type alias for `c_int`.[^windows]

[as_raw_fd]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.as_raw_fd
[`RawFd`]: https://doc.rust-lang.org/std/os/fd/type.RawFd.html

[^windows]: Unfortunately, none of these raw file descriptor operations will
    compile on Windows. This is a low enough level of detail that OS
    differences start to matter, and the Rust standard library doesn't try to
    abstract over them. To make code like this portable, we have to write it at
    least twice, using `#[cfg(unix)]` and `#[cfg(windows)]` to gate each
    implementation to a specific platform.

The `events` field is a collection of bitflags listing the events we want to
wait for. The most common events are [`POLLIN`], meaning input is available,
and [`POLLOUT`], meaning space is available in output buffers. For simplicity,
we'll assume that we only need to worry about blocking when reading from a
`TcpStream` or listening for new connections, so we'll set `events` to just
`POLLIN`.[^blocking_writes]

[`POLLIN`]: https://docs.rs/libc/latest/libc/constant.POLLIN.html
[`POLLOUT`]: https://docs.rs/libc/latest/libc/constant.POLLOUT.html

[^blocking_writes]: The size of the kernel write buffer for a `TcpStream` is
    measured in kilobytes, and our examples only write a handful of bytes, so
    realistically our writes will never block. This is another shortcut, but
    not quite as big of a shortcut as our treatment of `TcpStream::connect`
    above.

[`TcpStream::connect`]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.connect
[`getaddrinfo`]: https://man7.org/linux/man-pages/man3/getaddrinfo.3.html
[tokio_dns]: https://github.com/tokio-rs/tokio/blob/tokio-1.40.0/tokio/src/net/addr.rs#L182-L184

The `revents` field ("returned events") is similar but used for output rather
than input. After `poll` returns, the bits in this field indicate whether the
corresponding descriptor was one of the ones that caused the wakeup. We could
use this to poll only the specific tasks that the wakeup is for, but for
simplicity we'll ignore this field and poll every task every time we wake up.

To get file descriptors from the futures that own them to the main loop, we
need another global `Vec`:

```rust
static POLL_FDS: Mutex<Vec<(RawFd, Waker)>> = Mutex::new(Vec::new());
```

Now our `TcpAccept` and `Copy` futures can push into that `Vec`. Here's the
change in `TcpAccept`:

```rust
LINK: Playground playground://async_playground/client_server_poll.rs
HIGHLIGHT: 2-4
Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    let raw_fd = self.listener.as_raw_fd();
    let waker = context.waker().clone();
    POLL_FDS.lock().unwrap().push((raw_fd, waker));
    Poll::Pending
}
```

Finally, we can call `poll` in the main loop. TODO: Say so much more.

```rust
LINK: Playground playground://async_playground/client_server_poll.rs
// All tasks are either sleeping or blocked on IO. Use libc::poll to wait
// for IO on any of the POLL_FDS. If there are any WAKE_TIMES, use the
// earliest as a timeout.
let mut poll_fds = POLL_FDS.lock().unwrap();
let mut poll_structs = Vec::new();
for (raw_fd, _waker) in poll_fds.iter() {
    poll_structs.push(libc::pollfd {
        fd: *raw_fd,
        events: libc::POLLIN, // "poll input": wake when readable
        revents: 0,           // return field, unused
    });
}
let mut wake_times = WAKE_TIMES.lock().unwrap();
let timeout_ms = if let Some(time) = wake_times.keys().next() {
    let duration = time.saturating_duration_since(Instant::now());
    duration.as_millis() as libc::c_int
} else {
    -1 // infinite timeout
};
let poll_error_code = unsafe {
    libc::poll(
        poll_structs.as_mut_ptr(),
        poll_structs.len() as libc::nfds_t,
        timeout_ms,
    )
};
if poll_error_code == -1 {
    panic!("libc::poll failed: {}", io::Error::last_os_error());
}
// Invoke Wakers from WAKE_TIMES if their time has come.
while let Some(entry) = wake_times.first_entry() {
    if *entry.key() <= Instant::now() {
        entry.remove().into_iter().for_each(Waker::wake);
    } else {
        break;
    }
}
// Invoke all Wakers from POLL_FDS. This might wake futures that aren't
// ready yet, but if so they'll register another wakeup. It's inefficient
// but allowed.
for (_raw_fd, waker) in poll_fds.drain(..) {
    waker.wake();
}
```

Done?

---

<div class="prev-next-arrows">
    <div><a href="async_tasks.html">← Part Two: Tasks</a></div>
    <div class="space"> </div><div>
</div>
