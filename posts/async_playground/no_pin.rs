use std::collections::BTreeMap;
use std::sync::Mutex;
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

#[derive(Copy, Clone)]
struct Waker {}

impl Waker {
    fn wake(self) {}
}

static WAKERS: Mutex<BTreeMap<Instant, Vec<Waker>>> = Mutex::new(BTreeMap::new());

struct SleepFuture {
    wake_time: Instant,
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        if self.wake_time <= Instant::now() {
            Poll::Ready(())
        } else {
            let mut wakers_tree = WAKERS.lock().unwrap();
            let wakers_vec = wakers_tree.entry(self.wake_time).or_default();
            wakers_vec.push(context.waker().clone());
            Poll::Pending
        }
    }
}

fn sleep(duration: Duration) -> SleepFuture {
    let wake_time = Instant::now() + duration;
    SleepFuture { wake_time }
}

struct JobFuture {
    sleep_future: SleepFuture,
    n: u64,
}

impl Future for JobFuture {
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

fn job(n: u64) -> JobFuture {
    let sleep_future = sleep(Duration::from_secs(1));
    JobFuture { sleep_future, n }
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
        futures.push(job(n));
    }
    let mut main_future = join_all(futures);
    while main_future.poll(&mut Context {}).is_pending() {
        let mut wakers_tree = WAKERS.lock().unwrap();
        let next_wake = wakers_tree.keys().next().expect("sleep forever?");
        std::thread::sleep(next_wake.duration_since(Instant::now()));
        while let Some(entry) = wakers_tree.first_entry() {
            if *entry.key() <= Instant::now() {
                entry.remove().into_iter().for_each(Waker::wake);
            } else {
                break;
            }
        }
    }
}
