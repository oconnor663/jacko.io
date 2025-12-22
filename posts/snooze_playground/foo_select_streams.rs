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
    let mut stream1 = pin!(stream::once(foo()));
    let mut stream2 = pin!(stream::once(foo()));
    select! {
        _ = stream1.next() => {}
        _ = stream2.next() => {}
    }
    println!("We make it here...");
    foo().await;
    println!("...but not here!");
}
