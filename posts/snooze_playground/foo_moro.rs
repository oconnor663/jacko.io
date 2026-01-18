use std::time::Duration;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    println!("Is this actually concurrent?");
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    println!("Yes it is!");
}

#[tokio::main]
async fn main() {
    moro::async_scope!(|scope| {
        scope.spawn(async {
            scope.spawn(foo());
            foo().await;
        });
        foo().await;
    })
    .await;
}
