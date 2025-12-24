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
    let future1 = pin!(foo());
    let future2 = pin!(foo());
    select! {
        _ = future1 => {
            println!("We make it here...");
            foo().await;
            println!("...but not here!");
        }
        _ = future2 => {
            println!("Or maybe we make it here...");
            foo().await;
            println!("...but not here!");
        }
    }
}
