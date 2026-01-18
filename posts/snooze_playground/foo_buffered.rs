use futures::StreamExt;
use std::time::Duration;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    futures::stream::iter([foo(), foo()])
        .buffered(2)
        .for_each(async |_| {
            println!("We make it here...");
            foo().await;
            println!("...but not here!");
        })
        .await;
}
