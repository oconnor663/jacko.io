use tokio::select;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

static LOCK: Mutex<()> = Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    select! {
        _ = foo() => {}
        _ = sleep(Duration::from_millis(5)) => {}
    }
    foo().await;
    println!(
        "Passing the future to `select!` by value fixes the deadlock."
    );
}
