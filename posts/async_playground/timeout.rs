use futures::future;
use rand::prelude::*;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

async fn job(_n: u64) {
    let mut rng = rand::thread_rng();
    let sleep_seconds = rng.gen_range(0.0..100.0);
    tokio::time::sleep(Duration::from_secs_f32(sleep_seconds)).await;
    println!("job finished in {sleep_seconds:.3} seconds");
}

struct Timeout<F> {
    sleep: Pin<Box<tokio::time::Sleep>>,
    inner: Pin<Box<F>>,
}

impl<F: Future> Future for Timeout<F> {
    type Output = Option<F::Output>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<Self::Output> {
        if self.sleep.as_mut().poll(context).is_ready() {
            Poll::Ready(None)
        } else {
            match self.inner.as_mut().poll(context) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(output) => Poll::Ready(Some(output)),
            }
        }
    }
}

fn timeout<F: Future>(inner: F, duration: Duration) -> Timeout<F> {
    Timeout {
        sleep: Box::pin(tokio::time::sleep(duration)),
        inner: Box::pin(inner),
    }
}

#[tokio::main]
async fn main() {
    println!("Start with a thousand jobs. Each one does a random sleep,");
    println!("between 0 and 100 seconds. Time out after 1 second, so on");
    println!("average only 10 jobs will finish.\n");
    let mut futures = Vec::new();
    for n in 1..=1_000 {
        futures.push(job(n));
    }
    let all = future::join_all(futures);
    timeout(all, Duration::from_secs(1)).await;
}
