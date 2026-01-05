use futures::poll;
use std::pin::pin;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

static LOCK: Mutex<()> = Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    let future1 = pin!(foo());
    _ = poll!(future1);
    println!("We make it here...");
    foo().await;
    println!("...but not here!");
}
