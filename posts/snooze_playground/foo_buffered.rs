use futures::StreamExt;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

static LOCK: Mutex<()> = Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    sleep(Duration::from_millis(1)).await;
}

#[tokio::main]
async fn main() {
    futures::stream::iter([foo(), foo()])
        .buffered(2)
        .for_each(|_| async {
            foo().await;
        })
        .await;
}
