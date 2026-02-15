use futures::stream;
use std::time::Duration;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    join_me_maybe::join!(
        _ in stream::once(foo()) => {},
        // This arm's body runs concurrently with the stream above.
        _ = tokio::time::sleep(Duration::from_millis(5)) => foo().await,
    );
}
