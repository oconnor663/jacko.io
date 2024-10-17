use futures::future;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

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
        if !self.started {
            println!("start {}", self.n);
            self.started = true;
        }
        let before = Instant::now();
        let poll_result = self.sleep.as_mut().poll(context);
        let duration = Instant::now() - before;
        println!("Sleep::poll returned {poll_result:?} in {duration:?}.");
        if poll_result.is_pending() {
            Poll::Pending
        } else {
            println!("end {}", self.n);
            Poll::Ready(())
        }
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
