use futures::future;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

struct JobFuture {
    n: u64,
}

impl Future for JobFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _context: &mut Context) -> Poll<()> {
        // This version never returns Pending, so self.started isn't necessary.
        println!("start {}", self.n);
        std::thread::sleep(Duration::from_secs(1)); // Oops!
        println!("end {}", self.n);
        Poll::Ready(())
    }
}

fn job(n: u64) -> JobFuture {
    JobFuture { n }
}

#[tokio::main]
async fn main() {
    println!("Run a thousand jobs at the same time...");
    println!("\n...but something's not right...\n");
    let mut futures = Vec::new();
    for n in 1..=1_000 {
        futures.push(job(n));
    }
    future::join_all(futures).await;
}
