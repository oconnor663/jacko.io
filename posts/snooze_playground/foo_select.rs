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
    let mut future1 = pin!(foo());
    let mut future2 = pin!(foo());
    select! {
        _ = &mut future1 => {}
        _ = &mut future2 => {}
    }
    println!("We make it here...");
    foo().await;
    println!("...but not here!");
}
