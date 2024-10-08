use futures::future;
use std::time::{Duration, Instant};

async fn foo(_n: u64) {
    // Don't print. A million prints is too much output for the Playground.
    tokio::time::sleep(Duration::from_secs(1)).await;
}

#[tokio::main]
async fn main() {
    let start = Instant::now();
    let mut futures = Vec::new();
    for n in 1..=1_000_000 {
        futures.push(foo(n));
    }
    let joined_future = future::join_all(futures);
    joined_future.await;
    let time = Instant::now() - start;
    println!("time: {:.3} seconds", time.as_secs_f32());
}
