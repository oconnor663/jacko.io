use futures::future;
use std::time::Duration;

async fn job(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
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
