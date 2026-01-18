use futures::{StreamExt, stream};
use std::pin::pin;
use std::time::Duration;
use tokio::select;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    let mut stream = pin!(stream::once(foo()));
    select! {
        _ = stream.next() => {}
        _ = tokio::time::sleep(Duration::from_millis(5)) => {}
    }
    println!("We make it here...");
    foo().await;
    println!("...but not here!");
}
