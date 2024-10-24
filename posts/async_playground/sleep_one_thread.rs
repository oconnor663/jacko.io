use futures::future;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::LazyLock;
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

static WAKER_SENDER: LazyLock<Sender<(Instant, Waker)>> =
    LazyLock::new(|| {
        let (sender, receiver) = channel::<(Instant, Waker)>();
        // Kick off the waker thread the first time this sender is used.
        thread::spawn(move || {
            // A sorted multimap of wake times and wakers. The soonest wake time will be first.
            let mut wake_times = BTreeMap::<Instant, Vec<Waker>>::new();
            loop {
                // Wait to receive a new (wake_time, waker) pair. If we already have one or more
                // wakers, wait with a timeout, waking up at the earliest known wake time. Otherwise,
                // wait with no timeout.
                let new_pair = if let Some((first_wake_time, _)) =
                    wake_times.first_key_value()
                {
                    let timeout = first_wake_time
                        .saturating_duration_since(Instant::now());
                    match receiver.recv_timeout(timeout) {
                        Ok((wake_time, waker)) => Some((wake_time, waker)),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => {
                            unreachable!()
                        }
                    }
                } else {
                    match receiver.recv() {
                        Ok((wake_time, waker)) => Some((wake_time, waker)),
                        Err(_) => unreachable!(),
                    }
                };
                // If we got a waker pair above (i.e. we didn't time out), add it to the map.
                if let Some((wake_time, waker)) = new_pair {
                    let wakers_vec =
                        wake_times.entry(wake_time).or_default();
                    wakers_vec.push(waker.clone());
                }
                // Loop over all the wakers whose wake time has passed, removing them from the map and
                // invoking them.
                while let Some(entry) = wake_times.first_entry() {
                    if *entry.key() <= Instant::now() {
                        entry.remove().into_iter().for_each(Waker::wake);
                    } else {
                        break;
                    }
                }
            }
        });
        sender
    });

fn sleep(duration: Duration) -> Sleep {
    let wake_time = Instant::now() + duration;
    Sleep { wake_time }
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
            let waker_pair = (self.wake_time, context.waker().clone());
            WAKER_SENDER.send(waker_pair).unwrap();
            Poll::Pending
        }
    }
}

async fn foo(n: u64) {
    println!("start {n}");
    sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}

#[tokio::main]
async fn main() {
    let mut futures = Vec::new();
    for n in 1..=1_000 {
        futures.push(foo(n));
    }
    let joined_future = future::join_all(futures);
    joined_future.await;
}
