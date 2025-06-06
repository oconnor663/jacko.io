use futures::future;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

fn sleep(duration: Duration) -> Sleep {
    let wake_time = Instant::now() + duration;
    Sleep { wake_time }
}

struct Sleep {
    wake_time: Instant,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
        if Instant::now() >= self.wake_time {
            Poll::Ready(())
        } else {
            // DELETED LINE: context.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

async fn foo(n: u64) {
    println!("start {n}");
    sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}

fn main() {
    println!("These jobs never finish...");
    let mut futures = Vec::new();
    for n in 1..=100 {
        futures.push(foo(n));
    }
    let mut joined_future = Box::pin(future::join_all(futures));
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    while joined_future.as_mut().poll(&mut context).is_pending() {
        // Busy loop!
    }
}
