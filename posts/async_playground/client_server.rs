use std::collections::BTreeMap;
use std::future::Future;
use std::io;
use std::io::prelude::*;
use std::mem;
use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, RawFd};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

static WAKE_TIMES: Mutex<BTreeMap<Instant, Vec<Waker>>> = Mutex::new(BTreeMap::new());

struct Sleep {
    wake_time: Instant,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if Instant::now() >= self.wake_time {
            Poll::Ready(())
        } else {
            let mut wake_times = WAKE_TIMES.lock().unwrap();
            let wakers_vec = wake_times.entry(self.wake_time).or_default();
            wakers_vec.push(context.waker().clone());
            Poll::Pending
        }
    }
}

fn sleep(duration: Duration) -> Sleep {
    let wake_time = Instant::now() + duration;
    Sleep { wake_time }
}

type DynFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

static NEW_TASKS: Mutex<Vec<DynFuture>> = Mutex::new(Vec::new());

enum JoinState<T> {
    Unawaited,
    Awaited(Waker),
    Ready(T),
    Done,
}

struct JoinHandle<T> {
    state: Arc<Mutex<JoinState<T>>>,
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<T> {
        let mut guard = self.state.lock().unwrap();
        // Use JoinState::Done as a placeholder, to take ownership of T.
        match mem::replace(&mut *guard, JoinState::Done) {
            JoinState::Ready(value) => Poll::Ready(value),
            JoinState::Unawaited | JoinState::Awaited(_) => {
                // Replace the previous Waker, if any. We only need the most recent one.
                *guard = JoinState::Awaited(context.waker().clone());
                Poll::Pending
            }
            JoinState::Done => unreachable!("polled again after Ready"),
        }
    }
}

async fn wrap_with_join_state<F: Future>(future: F, join_state: Arc<Mutex<JoinState<F::Output>>>) {
    let value = future.await;
    let mut guard = join_state.lock().unwrap();
    if let JoinState::Awaited(waker) = &*guard {
        waker.wake_by_ref();
    }
    *guard = JoinState::Ready(value)
}

fn spawn<F, T>(future: F) -> JoinHandle<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let join_state = Arc::new(Mutex::new(JoinState::Unawaited));
    let join_handle = JoinHandle {
        state: Arc::clone(&join_state),
    };
    let task = Box::pin(wrap_with_join_state(future, join_state));
    NEW_TASKS.lock().unwrap().push(task);
    join_handle
}

// In production we'd use AtomicBool instead of Mutex<bool>.
struct AwakeFlag(Mutex<bool>);

impl AwakeFlag {
    fn check_and_clear(&self) -> bool {
        let mut guard = self.0.lock().unwrap();
        let check = *guard;
        *guard = false;
        check
    }
}

impl Wake for AwakeFlag {
    fn wake(self: Arc<Self>) {
        *self.0.lock().unwrap() = true;
    }
}

static POLL_FDS: Mutex<Vec<(RawFd, Waker)>> = Mutex::new(Vec::new());

async fn tcp_bind(address: &str) -> io::Result<TcpListener> {
    // XXX: This is technically blocking. Assume it returns quickly.
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

struct TcpAccept<'a> {
    listener: &'a TcpListener,
}

impl<'a> Future for TcpAccept<'a> {
    type Output = io::Result<TcpStream>;

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<io::Result<TcpStream>> {
        match self.listener.accept() {
            Ok((stream, _)) => {
                let result = stream.set_nonblocking(true);
                Poll::Ready(result.and(Ok(stream)))
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

fn tcp_accept(listener: &TcpListener) -> TcpAccept {
    TcpAccept { listener }
}

struct Copy<'a, R, W> {
    reader: &'a mut R,
    writer: &'a mut W,
}

impl<'a, R: Read + AsRawFd, W: Write> Future for Copy<'a, R, W> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<io::Result<()>> {
        let Copy { reader, writer } = &mut *self.as_mut();
        match io::copy(reader, writer) {
            Ok(_) => Poll::Ready(Ok(())),
            // XXX: Assume that the writer will never block.
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                let raw_fd = self.reader.as_raw_fd();
                let waker = context.waker().clone();
                POLL_FDS.lock().unwrap().push((raw_fd, waker));
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

fn copy<'a, R, W>(reader: &'a mut R, writer: &'a mut W) -> Copy<'a, R, W> {
    Copy { reader, writer }
}

async fn foo_response(n: u64, mut socket: TcpStream) -> io::Result<()> {
    // XXX: Assume the write buffer is large enough that we don't need to handle WouldBlock.
    // Using format! instead of write! avoids breaking up lines across multiple writes. This is
    // easier than doing line buffering on the client side.
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
        spawn(async move { foo_response(n, socket).await.unwrap() });
        n += 1;
    }
}

async fn foo_request() -> io::Result<()> {
    let mut socket = tcp_connect("localhost:8000").await?;
    copy(&mut socket, &mut io::stdout()).await?;
    Ok(())
}

async fn async_main() -> io::Result<()> {
    // Open the listener here, to avoid racing against the server thread.
    let listener = tcp_bind("0.0.0.0:8000").await?;
    spawn(async { server_main(listener).await.unwrap() });
    let mut task_handles = Vec::new();
    for _ in 1..=10 {
        task_handles.push(spawn(foo_request()));
    }
    for handle in task_handles {
        handle.await?;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let awake_flag = Arc::new(AwakeFlag(Mutex::new(false)));
    let waker = Waker::from(Arc::clone(&awake_flag));
    let mut context = Context::from_waker(&waker);
    let mut main_task = Box::pin(async_main());
    let mut other_tasks: Vec<DynFuture> = Vec::new();
    loop {
        // Poll the main task and exit immediately if it's done.
        if let Poll::Ready(result) = main_task.as_mut().poll(&mut context) {
            return result;
        }
        // Poll other tasks and remove any that are Ready.
        let is_pending = |task: &mut DynFuture| task.as_mut().poll(&mut context).is_pending();
        other_tasks.retain_mut(is_pending);
        // Some tasks might have spawned new tasks. Pop from NEW_TASKS until it's empty. Note that
        // we can't use while-let here, because that would keep NEW_TASKS locked in the loop body.
        // See https://fasterthanli.me/articles/a-rust-match-made-in-hell.
        loop {
            let Some(mut task) = NEW_TASKS.lock().unwrap().pop() else {
                break;
            };
            // Poll each new task now, instead of waiting for the next iteration of the main loop,
            // to let them register wakeups. Drop the ones that return Ready. This poll can also
            // spawn more tasks, so it's important that NEW_TASKS isn't locked here.
            if task.as_mut().poll(&mut context).is_pending() {
                other_tasks.push(task);
            }
        }
        // Some tasks might wake other tasks. Re-poll if the AwakeFlag has been set. Polling
        // futures that aren't ready yet is inefficient but allowed.
        if awake_flag.check_and_clear() {
            continue;
        }
        // All tasks are either sleeping or blocked on IO. Use libc::poll to wait for IO on any of
        // the POLL_FDS. If there are any WAKE_TIMES, use the earliest as a timeout.
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
        // Invoke all Wakers from POLL_FDS. This might wake futures that aren't ready yet, but if
        // so they'll register another wakeup. It's inefficient but allowed.
        poll_fds.drain(..).map(|pair| pair.1).for_each(Waker::wake);
    }
}
