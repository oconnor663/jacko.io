use std::time::Duration;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[tokio::main]
async fn main() {
    // If we didn't capture `_output` here, this example wouldn't deadlock.
    // But this `select` function (not the `select!` macro) returns a tuple
    // of one output and one future (the "loser"). If we don't either let
    // that future drop or keep polling it, then we're snoozing it.
    let _output = futures::future::select(
        Box::pin(foo()),
        Box::pin(tokio::time::sleep(Duration::from_millis(5))),
    )
    .await;
    println!("We make it here...");
    foo().await;
    println!("...but not here!");
}
