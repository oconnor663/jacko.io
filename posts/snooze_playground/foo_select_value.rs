use std::time::Duration;
use tokio::select;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    select! {
        _ = foo() => {}
        _ = tokio::time::sleep(Duration::from_millis(5)) => {}
    }
    foo().await;
    println!(
        "Passing the future to `select!` by value fixes the deadlock."
    );
}
