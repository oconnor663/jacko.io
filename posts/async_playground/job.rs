use futures::future;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

struct JobFuture {
    n: u64,
    started: bool,
    sleep_future: Pin<Box<tokio::time::Sleep>>,
}

impl Future for JobFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if !self.started {
            println!("start {}", self.n);
            self.started = true;
        }
        if self.sleep_future.as_mut().poll(context).is_pending() {
            Poll::Pending
        } else {
            println!("end {}", self.n);
            Poll::Ready(())
        }
    }
}

fn job(n: u64) -> JobFuture {
    let sleep_future = tokio::time::sleep(Duration::from_secs(1));
    JobFuture {
        n,
        started: false,
        sleep_future: Box::pin(sleep_future),
    }
}

#[tokio::main]
async fn main() {
    println!("Run three jobs, one at a time...\n");
    job(1).await;
    job(2).await;
    job(3).await;

    println!("\nRun three jobs at the same time...\n");
    let mut futures = Vec::new();
    for n in 1..=3 {
        futures.push(job(n));
    }
    future::join_all(futures).await;
}
