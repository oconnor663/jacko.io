use std::time::Duration;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    join_me_maybe::join!(
        _ = foo() => return,
        // Do some periodic background work while the first `foo` is
        // running. Without the `maybe` keyword, `join` tries to run both
        // arms to completion. This arm is an infinite loop, but the
        // `return` above short-circuits the whole function.
        async {
            loop {
                tokio::time::sleep(Duration::from_millis(5)).await;
                foo().await;
            }
        }
    );
}
