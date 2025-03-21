use futures::future;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

async fn foo(_n: u64) {
    let sleep_seconds = rand::random_range(0.0..100.0);
    tokio::time::sleep(Duration::from_secs_f32(sleep_seconds)).await;
    println!("foo finished in {sleep_seconds:.3} seconds");
}

struct Timeout<F> {
    sleep: Pin<Box<tokio::time::Sleep>>,
    inner: Pin<Box<F>>,
}

impl<F: Future> Future for Timeout<F> {
    type Output = Option<F::Output>;

    fn poll(
        mut self: Pin<&mut Self>,
        context: &mut Context,
    ) -> Poll<Self::Output> {
        // Check whether time is up.
        if self.sleep.as_mut().poll(context).is_ready() {
            return Poll::Ready(None);
        }
        // Check whether the inner future is finished.
        if let Poll::Ready(output) = self.inner.as_mut().poll(context) {
            return Poll::Ready(Some(output));
        }
        // Still waiting.
        Poll::Pending
    }
}

fn timeout<F: Future>(duration: Duration, inner: F) -> Timeout<F> {
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
        futures.push(foo(n));
    }
    let all = future::join_all(futures);
    timeout(Duration::from_secs(1), all).await;
}
