use std::pin::{Pin, pin};
use std::task::Poll;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

static LOCK: Mutex<()> = Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    sleep(Duration::from_millis(1)).await;
}

struct PollOnce<'a, Fut>(Pin<&'a mut Fut>);

impl<'a, Fut: Future> Future for PollOnce<'a, Fut> {
    type Output = ();

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        _ = self.0.as_mut().poll(cx);
        Poll::Ready(())
    }
}

#[tokio::main]
async fn main() {
    let future = pin!(foo());
    PollOnce(future).await;
    println!("We make it here...");
    foo().await;
    println!("...but not here!");
}
