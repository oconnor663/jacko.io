use futures::StreamExt;
use futures::lock::Mutex;
use std::sync::LazyLock;
use tokio::time::{Duration, sleep};

// This mutex implementation is unfair! (It also doesn't have a const new
// function, so we need a `LazyLock` to initialize it.)
static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

async fn foo() {
    let guard = LOCK.lock().await;
    sleep(Duration::from_millis(10)).await;
    drop(guard);
    // This second sleep allows the second `foo` to acquire the lock before
    // the first `foo` finishes below. If you comment it out, the deadlock
    // no longer appears. (The original version of this example using a
    // `tokio::sync::Mutex` didn't need this, because that mutex is fair.)
    sleep(Duration::from_millis(5)).await;
}

#[tokio::main]
async fn main() {
    futures::stream::iter([foo(), foo()])
        .buffered(2)
        .for_each(async |_| {
            println!("We make it here...");
            foo().await;
            println!("...but not here!");
        })
        .await;
}
