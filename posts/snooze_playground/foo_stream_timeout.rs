use futures::{StreamExt, stream};
use std::pin::pin;
use std::time::Duration;
use tokio::time::timeout;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    let mut stream = pin!(stream::once(foo()));
    _ = timeout(Duration::from_millis(5), stream.next()).await;
    println!("We make it here...");
    foo().await;
    println!("...but not here!");
}
