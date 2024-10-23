use std::time::Duration;

async fn foo(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}

#[tokio::main]
async fn main() {
    println!("These jobs don't run at the same time...");
    let mut futures = Vec::new();
    for n in 1..=10 {
        futures.push(foo(n));
    }
    for future in futures {
        future.await; // Oops!
    }
}
