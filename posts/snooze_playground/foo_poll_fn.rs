use std::pin::pin;
use std::task::Poll;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

static LOCK: Mutex<()> = Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    sleep(Duration::from_millis(1)).await;
}

#[tokio::main]
async fn main() {
    let future1 = foo();
    let mut future2 = pin!(foo());
    std::future::poll_fn(|cx| {
        _ = future2.as_mut().poll(cx);
        Poll::Ready(())
    })
    .await;
    future1.await;
    future2.await;
}
