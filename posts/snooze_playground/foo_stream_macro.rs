use futures::{FutureExt, StreamExt};
use std::pin::pin;
use std::time::Duration;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    let mut stream = pin!(async_stream::stream! {
        let mut foo1 = pin!(foo().fuse());
        let mut foo2 = pin!(foo().fuse());
        loop {
            futures::select! {
                x = &mut foo1 => {
                    yield x;
                }
                x = &mut foo2 => {
                    yield x;
                }
            }
        }
    });
    while let Some(_) = stream.next().await {
        println!("We make it here...");
        foo().await;
        println!("...but not here!");
    }
}
