use futures::poll;
use std::pin::pin;
use std::time::Duration;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    let future1 = pin!(foo());
    _ = poll!(future1);
    println!("We make it here...");
    foo().await;
    println!("...but not here!");
}
