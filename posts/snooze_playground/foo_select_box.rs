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
    let future = Box::pin(foo());
    select! {
        _ = future => {}
        _ = sleep(Duration::from_millis(1)) => {}
    }
    foo().await;
    println!(
        "Replacing `pin!` with `Box::pin` fixes the deadlock in this case."
    );
}
