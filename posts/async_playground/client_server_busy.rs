use std::collections::BTreeMap;
use std::future::Future;
use std::io;
use std::io::prelude::*;
use std::mem;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
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

async fn accept(listener: &mut TcpListener) -> io::Result<(TcpStream, SocketAddr)> {
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
    })
    .await
}

async fn write_all(mut buf: &[u8], stream: &mut TcpStream) -> io::Result<()> {
    std::future::poll_fn(|context| {
        while !buf.is_empty() {
            match stream.write(&buf) {
                Ok(n) if n == 0 => {
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
    })
    .await
}

async fn print_all(stream: &mut TcpStream) -> io::Result<()> {
    let mut buf = [0; 1024];
    std::future::poll_fn(|context| {
        loop {
            match stream.read(&mut buf) {
                Ok(n) if n == 0 => return Poll::Ready(Ok(())), // EOF
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
    })
    .await
}

async fn one_response(mut socket: TcpStream, n: u64) -> io::Result<()> {
    // Using format! instead of write! avoids breaking up lines across multiple writes. This is
    // easier than doing line buffering on the client side.
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

async fn client_main() -> io::Result<()> {
    // XXX: Assume that connect() returns quickly.
    let mut socket = TcpStream::connect("localhost:8000")?;
    socket.set_nonblocking(true)?;
    print_all(&mut socket).await?;
    Ok(())
}

async fn async_main() -> io::Result<()> {
    // Avoid a race between bind and connect by binding first.
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
        // Sleep until the next Waker is scheduled and then invoke Wakers that are ready.
        let mut wake_times = WAKE_TIMES.lock().unwrap();
        let next_wake = wake_times.keys().next().expect("sleep forever?");
        thread::sleep(next_wake.saturating_duration_since(Instant::now()));
        while let Some(entry) = wake_times.first_entry() {
            if *entry.key() <= Instant::now() {
                entry.remove().into_iter().for_each(Waker::wake);
            } else {
                break;
            }
        }
    }
}