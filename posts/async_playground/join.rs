use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

async fn job(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}

struct JoinFuture<F> {
    futures: Vec<Pin<Box<F>>>,
}

impl<F: Future> Future for JoinFuture<F> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        let is_pending = |future: &mut Pin<Box<F>>| future.as_mut().poll(context).is_pending();
        self.futures.retain_mut(is_pending);
        if self.futures.is_empty() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

fn join_all<F: Future>(futures: Vec<F>) -> JoinFuture<F> {
    JoinFuture {
        futures: futures.into_iter().map(Box::pin).collect(),
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
    join_all(futures).await;
}
