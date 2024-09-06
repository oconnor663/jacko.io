use futures::future;
use std::time::Duration;

async fn foo(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}

#[tokio::main]
async fn main() {
    let mut futures = Vec::new();
    for n in 1..=10 {
        futures.push(foo(n));
    }
    println!("Joining the foo futures...");
    let joined_future = future::join_all(futures);
    println!("That didn't take any time.");

    tokio::time::sleep(Duration::from_secs(1)).await;

    println!("Awaiting the joined futures...");
    joined_future.await;
}