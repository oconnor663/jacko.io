use futures::future;
use std::thread;
use std::time::Duration;

async fn foo(n: u64) {
    println!("start {n}");
    thread::sleep(Duration::from_secs(1)); // Oops!
    println!("end {n}");
}

#[tokio::main]
async fn main() {
    println!("These jobs don't run at the same time...");
    let mut futures = Vec::new();
    for n in 1..=10 {
        futures.push(foo(n));
    }
    let joined_future = future::join_all(futures);
    joined_future.await;
}
