use futures::future;
use std::time::Duration;

async fn job(n: u64) {
    println!("start {n}");
    std::thread::sleep(Duration::from_secs(1)); // Oops!
    println!("end {n}");
}

#[tokio::main]
async fn main() {
    println!("Run three jobs at the same time...");
    println!("...but something's not right...\n");
    let mut futures = Vec::new();
    for n in 1..=3 {
        futures.push(job(n));
    }
    future::join_all(futures).await;
}
