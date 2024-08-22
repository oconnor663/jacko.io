use futures::future;
use std::time::Duration;

async fn foo(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}

#[tokio::main]
async fn main() {
    println!("Run a thousand jobs at the same time...\n");
    let mut futures = Vec::new();
    for n in 1..=1_000 {
        futures.push(foo(n));
    }
    future::join_all(futures).await;
}
