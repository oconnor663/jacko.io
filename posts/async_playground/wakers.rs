use futures::future;
use futures::task::noop_waker_ref;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

static WAKERS: Mutex<BTreeMap<Instant, Vec<Waker>>> = Mutex::new(BTreeMap::new());

struct SleepFuture {
    wake_time: Instant,
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
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

async fn foo(n: u64) {
    println!("start {n}");
    sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}

fn main() {
    let mut futures = Vec::new();
    for n in 1..=10 {
        futures.push(foo(n));
    }
    let mut main_future = Box::pin(future::join_all(futures));
    let mut context = Context::from_waker(noop_waker_ref());
    while main_future.as_mut().poll(&mut context).is_pending() {
        let mut wakers_tree = WAKERS.lock().unwrap();
        let next_wake = wakers_tree.keys().next().expect("sleep forever?");
        std::thread::sleep(next_wake.saturating_duration_since(Instant::now()));
        while let Some(entry) = wakers_tree.first_entry() {
            if *entry.key() <= Instant::now() {
                entry.remove().into_iter().for_each(Waker::wake);
            } else {
                break;
            }
        }
    }
}
