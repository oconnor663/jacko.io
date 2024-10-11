use futures::future;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

thread_local! {
    static WAKE_TIMES: RefCell<BTreeMap<Instant, Vec<Waker>>> = RefCell::new(BTreeMap::new());
}

struct Sleep {
    wake_time: Instant,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if Instant::now() >= self.wake_time {
            Poll::Ready(())
        } else {
            WAKE_TIMES.with_borrow_mut(|wake_times| {
                let wakers_vec =
                    wake_times.entry(self.wake_time).or_default();
                wakers_vec.push(context.waker().clone());
                Poll::Pending
            })
        }
    }
}

fn sleep(duration: Duration) -> Sleep {
    let wake_time = Instant::now() + duration;
    Sleep { wake_time }
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
    let mut joined_future = Box::pin(future::join_all(futures));
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    while joined_future.as_mut().poll(&mut context).is_pending() {
        WAKE_TIMES.with_borrow_mut(|wake_times| {
            let next_wake =
                wake_times.keys().next().expect("sleep forever?");
            thread::sleep(
                next_wake.saturating_duration_since(Instant::now()),
            );
            while let Some(entry) = wake_times.first_entry() {
                if *entry.key() <= Instant::now() {
                    entry.remove().into_iter().for_each(Waker::wake);
                } else {
                    break;
                }
            }
        });
    }
}
