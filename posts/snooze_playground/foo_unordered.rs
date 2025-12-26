use futures::StreamExt;
use futures::stream::FuturesUnordered;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

static LOCK: Mutex<()> = Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    let mut futures = FuturesUnordered::new();
    futures.push(foo());
    futures.push(foo());
    while let Some(_) = futures.next().await {
        println!("We make it here...");
        foo().await;
        println!("...but not here!");
    }
}
