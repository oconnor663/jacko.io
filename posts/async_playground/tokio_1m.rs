use futures::future;
use std::time::{Duration, Instant};

async fn job(_n: u64) {
    // Don't print. A million prints is too much output for the Playground.
    tokio::time::sleep(Duration::from_secs(1)).await;
}

#[tokio::main]
async fn main() {
    println!("Run a million jobs at the same time...\n");
    let start = Instant::now();
    let mut futures = Vec::new();
    for n in 1..=1_000_000 {
        futures.push(job(n));
    }
    future::join_all(futures).await;
    let time = Instant::now() - start;
    println!("time: {:.3} seconds", time.as_secs_f32());
}
