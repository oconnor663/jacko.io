use futures::future;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

struct SleepFuture {
    wake_time: Instant,
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if self.wake_time <= Instant::now() {
            Poll::Ready(())
        } else {
            let wake_time = self.wake_time;
            let waker = context.waker().clone();
            std::thread::spawn(move || {
                std::thread::sleep(wake_time.saturating_duration_since(Instant::now()));
                waker.wake();
            });
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

#[tokio::main]
async fn main() {
    let mut futures = Vec::new();
    for n in 1..=1_000 {
        futures.push(foo(n));
    }
    future::join_all(futures).await;
}
