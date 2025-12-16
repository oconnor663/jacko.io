use futures::{StreamExt, stream};
use std::pin::pin;
use tokio::select;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

static LOCK: Mutex<()> = Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    sleep(Duration::from_millis(1)).await;
}

#[tokio::main]
async fn main() {
    let mut future1 = pin!(stream::once(foo()));
    let mut future2 = pin!(stream::once(foo()));
    select! {
        _ = future1.next() => foo().await,
        _ = future2.next() => foo().await,
    }
}
