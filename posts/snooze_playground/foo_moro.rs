use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

static LOCK: Mutex<()> = Mutex::const_new(());

async fn foo() {
    println!("Is this actually concurrent?");
    let _guard = LOCK.lock().await;
    sleep(Duration::from_millis(1)).await;
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
