use futures::future;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

fn foo(n: u64) -> Foo {
    let started = false;
    let duration = Duration::from_secs(1);
    let sleep = Box::pin(tokio::time::sleep(duration));
    Foo { n, started, sleep }
}

struct Foo {
    n: u64,
    started: bool,
    sleep: Pin<Box<tokio::time::Sleep>>,
}

impl Future for Foo {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        // EXTRA PRINT HERE
        println!("Foo {} polled", self.n);
        if !self.started {
            println!("start {}", self.n);
            self.started = true;
        }
        if self.sleep.as_mut().poll(context).is_pending() {
            return Poll::Pending;
        }
        println!("end {}", self.n);
        Poll::Ready(())
    }
}

#[tokio::main]
async fn main() {
    let mut futures = Vec::new();
    for n in 1..=10 {
        futures.push(foo(n));
    }
    let joined_future = future::join_all(futures);
    joined_future.await;
}
