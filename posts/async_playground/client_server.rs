use std::future::Future;
use std::io;
use std::io::prelude::*;
use std::mem;
use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, RawFd};
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

static WAKE_TIMES: Mutex<Vec<(Instant, Waker)>> = Mutex::new(Vec::new());

struct Sleep {
    wake_time: Instant,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if Instant::now() >= self.wake_time {
            Poll::Ready(())
        } else {
            let waker = context.waker().clone();
            WAKE_TIMES.lock().unwrap().push((self.wake_time, waker));
            Poll::Pending
        }
    }
}

fn sleep(duration: Duration) -> Sleep {
    let wake_time = Instant::now() + duration;
    Sleep { wake_time }
}

static POLL_FDS: Mutex<Vec<(RawFd, Waker)>> = Mutex::new(Vec::new());

struct TcpAccept<'a> {
    listener: &'a TcpListener,
}

impl<'a> Future for TcpAccept<'a> {
    type Output = io::Result<TcpStream>;

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<io::Result<TcpStream>> {
        match self.listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(true).expect("cannot fail");
                Poll::Ready(Ok(stream))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                let raw_fd = self.listener.as_raw_fd();
                let waker = context.waker().clone();
                POLL_FDS.lock().unwrap().push((raw_fd, waker));
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

fn accept(listener: &TcpListener) -> TcpAccept {
    TcpAccept { listener }
}

struct TcpReadLine<'a> {
    reader: &'a mut io::BufReader<TcpStream>,
    line: String,
}

impl<'a, 'b> Future for TcpReadLine<'a> {
    type Output = io::Result<Option<String>>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<io::Result<Option<String>>> {
        let Self { reader, line } = &mut *self;
        match reader.read_line(line) {
            Ok(n) => {
                if n > 0 {
                    Poll::Ready(Ok(Some(mem::take(line))))
                } else {
                    Poll::Ready(Ok(None))
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                let raw_fd = reader.get_ref().as_raw_fd();
                let waker = context.waker().clone();
                POLL_FDS.lock().unwrap().push((raw_fd, waker));
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

fn read_line(reader: &mut io::BufReader<TcpStream>) -> TcpReadLine {
    let line = String::new();
    TcpReadLine { reader, line }
}

async fn foo_response(n: u64, mut socket: TcpStream) -> io::Result<()> {
    // XXX: technically blocking, assumed to return quickly
    writeln!(&mut socket, "start {n}")?;
    sleep(Duration::from_secs(1)).await;
    writeln!(&mut socket, "end {n}")?;
    Ok(())
}

async fn server_main(listener: TcpListener) -> io::Result<()> {
    let mut n = 1;
    loop {
        let socket = accept(&listener).await?;
        spawn(async move { foo_response(n, socket).await.unwrap() });
        n += 1;
    }
}

async fn foo_request() -> io::Result<()> {
    // XXX: technically blocking, assumed to return quickly
    let socket = TcpStream::connect("localhost:8000")?;
    socket.set_nonblocking(true).expect("cannot fail");
    let mut reader = io::BufReader::new(socket);
    while let Some(line) = read_line(&mut reader).await? {
        print!("{}", line); // `line` includes a trailing newline
    }
    Ok(())
}

async fn async_main() {
    // Open the listener here, to avoid racing against the server thread.
    // XXX: technically blocking, assumed to return quickly
    let listener = TcpListener::bind("localhost:8000").unwrap();
    listener.set_nonblocking(true).expect("cannot fail");
    spawn(async { server_main(listener).await.unwrap() });
    for _ in 1..=10 {
        spawn(async { foo_request().await.unwrap() });
    }
}

type DynFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

static NEW_TASKS: Mutex<Vec<DynFuture>> = Mutex::new(Vec::new());

fn spawn<F: Future<Output = ()> + Send + 'static>(future: F) {
    NEW_TASKS.lock().unwrap().push(Box::pin(future));
}

fn main() {
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    let mut tasks: Vec<DynFuture> = vec![Box::pin(async_main())];
    loop {
        // Poll each task, removing any that are Ready.
        let is_pending = |task: &mut DynFuture| task.as_mut().poll(&mut context).is_pending();
        tasks.retain_mut(is_pending);
        // The tasks we just polled might've spawned new tasks. Pop from NEW_TASKS until it's
        // empty. Note that we can't use while-let here, because that would keep NEW_TASKS locked
        // in the loop body. See https://fasterthanli.me/articles/a-rust-match-made-in-hell.
        loop {
            let Some(mut task) = NEW_TASKS.lock().unwrap().pop() else {
                break;
            };
            // Poll each new task now, instead of waiting for the next iteration of the main loop,
            // to let them register wakeups. Drop the ones that return Ready. This poll can also
            // spawn more tasks, so it's important that NEW_TASKS isn't locked here.
            if task.as_mut().poll(&mut context).is_pending() {
                tasks.push(task);
            }
        }
        // If there are no tasks left, we're done. Note that this is different from Tokio.
        if tasks.is_empty() {
            break;
        }
        // Block on POLL_FDS until one of them is readable. If there are any SLEEP_WAKERS
        // scheduled, set a timeout for the next wakeup.
        let mut poll_fds = POLL_FDS.lock().unwrap();
        let mut poll_structs = Vec::new();
        for &(raw_fd, _) in poll_fds.iter() {
            poll_structs.push(libc::pollfd {
                fd: raw_fd,
                events: libc::POLLIN, // "poll input": wake when readable
                revents: 0,           // return field, unused
            });
        }
        let mut wake_times = WAKE_TIMES.lock().unwrap();
        let min_wake_time = wake_times.iter().map(|(time, _)| time).min();
        let timeout_ms = if let Some(wake_time) = min_wake_time {
            wake_time
                .saturating_duration_since(Instant::now())
                .as_millis() as libc::c_int
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
        assert_ne!(poll_error_code, -1, "libc::poll failed");
        // Drain POLL_FDS and WAKE_TIMES and invoke all their wakers. This might wake some futures
        // early, but they'll register another wakeup when we poll them.
        for (_, waker) in poll_fds.drain(..) {
            waker.wake();
        }
        for (_, waker) in wake_times.drain(..) {
            waker.wake();
        }
    }
}
