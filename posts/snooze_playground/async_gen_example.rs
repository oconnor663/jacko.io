#![feature(gen_blocks)]
#![feature(async_iterator)]

use std::async_iter::AsyncIterator;
use std::pin::{Pin, pin};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{Duration, sleep};

// a helper function similar to `StreamExt::next`
fn next<T>(
    mut iter: Pin<&mut impl AsyncIterator<Item = T>>,
) -> impl Future<Output = Option<T>> {
    futures::future::poll_fn(move |cx| iter.as_mut().poll_next(cx))
}

// An async generator function! As of December 2025, this syntax is
// nightly-only.
async gen fn double_each(mut channel: UnboundedReceiver<i32>) -> i32 {
    while let Some(n) = channel.recv().await {
        yield 2 * n;
    }
}

#[tokio::main]
async fn main() {
    // Open a channel and spawn a background task that sends a number into
    // it every second.
    let (send, recv) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        for i in 1..=10 {
            send.send(i).unwrap();
            sleep(Duration::from_secs(1)).await;
        }
    });
    // Pass the receive end of the channel to an `async gen` function that
    // doubles each element, and loop over the results.
    let mut stream = pin!(double_each(recv));
    while let Some(n) = next(stream.as_mut()).await {
        dbg!(n);
    }
}
