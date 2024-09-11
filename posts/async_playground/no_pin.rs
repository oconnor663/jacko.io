use std::collections::BTreeMap;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

trait Future {
    type Output;

    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output>;
}

enum Poll<T> {
    Pending,
    Ready(T),
}

impl<T> Poll<T> {
    fn is_pending(&self) -> bool {
        matches!(self, Poll::Pending)
    }
}

struct Context {}

impl Context {
    fn waker(&self) -> Waker {
        Waker {}
    }
}

#[derive(Clone)]
struct Waker {}

impl Waker {
    fn wake(self) {}
}

static WAKE_TIMES: Mutex<BTreeMap<Instant, Vec<Waker>>> = Mutex::new(BTreeMap::new());

struct Sleep {
    wake_time: Instant,
}

impl Future for Sleep {
    type Output = ();

    fn poll(&mut self, context: &mut Context) -> Poll<()> {
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

struct Foo {
    sleep_future: Sleep,
    n: u64,
}

impl Future for Foo {
    type Output = ();

    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        if self.sleep_future.poll(context).is_pending() {
            Poll::Pending
        } else {
            println!("{}", self.n);
            Poll::Ready(())
        }
    }
}

fn foo(n: u64) -> Foo {
    let sleep_future = sleep(Duration::from_secs(1));
    Foo { sleep_future, n }
}

struct JoinFuture<F> {
    futures: Vec<F>,
}

impl<F: Future> Future for JoinFuture<F> {
    type Output = ();

    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        let is_pending = |future: &mut F| future.poll(context).is_pending();
        self.futures.retain_mut(is_pending);
        if self.futures.is_empty() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

fn join_all<F: Future>(futures: Vec<F>) -> JoinFuture<F> {
    JoinFuture { futures }
}

fn main() {
    let mut futures = Vec::new();
    for n in 1..=1_000 {
        futures.push(foo(n));
    }
    let mut joined_future = join_all(futures);
    while joined_future.poll(&mut Context {}).is_pending() {
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
