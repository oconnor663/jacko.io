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

async fn job(n: u64) {
    sleep(Duration::from_secs(1)).await;
    println!("{n}");
}

fn main() {
    let mut futures = Vec::new();
    for n in 1..=1_000 {
        futures.push(job(n));
    }
    let mut main_future = Box::pin(future::join_all(futures));
    let mut context = Context::from_waker(noop_waker_ref());
    while main_future.as_mut().poll(&mut context).is_pending() {
        let mut wakers_tree = WAKERS.lock().unwrap();
        let next_wake = wakers_tree
            .keys()
            .next()
            .expect("OOPS! The main future is Pending but there's no wake time.");
        std::thread::sleep(next_wake.duration_since(Instant::now()));
        while let Some(entry) = wakers_tree.first_entry() {
            if *entry.key() <= Instant::now() {
                // OOPS: Skip invoking the wakers. This eventually leads to a panic above, because
                // JoinAll will return Pending without polling any of its children a second time.
                // NOTE: As of futures v0.3.30, you can "fix" this by reducing the number of jobs
                // to 30 or fewer. Below that threshold, JoinAll falls back to a simple
                // implementation that always polls its children.
                // https://docs.rs/futures/0.3.30/futures/future/fn.join_all.html#see-also
                // https://docs.rs/futures-util/0.3.30/src/futures_util/future/join_all.rs.html#35
                entry.remove();
            } else {
                break;
            }
        }
    }
}
