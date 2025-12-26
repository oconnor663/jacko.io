use std::pin::pin;
use std::task::Poll;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

static LOCK: Mutex<()> = Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    let mut future = pin!(foo());
    std::future::poll_fn(|cx| {
        _ = future.as_mut().poll(cx);
        Poll::Ready(())
    })
    .await;
    println!("We make it here...");
    foo().await;
    println!("...but not here!");
}
