use std::pin::pin;
use std::task::Poll;
use std::time::Duration;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    let mut future1 = pin!(foo());
    std::future::poll_fn(|cx| {
        _ = future1.as_mut().poll(cx);
        Poll::Ready(())
    })
    .await;
    println!("We make it here...");
    foo().await;
    println!("...but not here!");
}
