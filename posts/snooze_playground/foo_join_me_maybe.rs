use std::time::Duration;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    join_me_maybe::join!(
        foo(),
        // Do some periodic background work while the first `foo` is running. The `join` runs both
        // arms concurrently, but the `maybe` keyword means it doesn't wait for this arm to finish.
        maybe async {
            loop {
                tokio::time::sleep(Duration::from_millis(5)).await;
                foo().await;
            }
        }
    );
}
