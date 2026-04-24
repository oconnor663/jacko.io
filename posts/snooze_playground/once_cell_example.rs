use futures::{StreamExt, stream};
use std::pin::pin;
use tokio::sync::OnceCell;
use tokio::time::{Duration, timeout};

/// Fetch some Zen wisdom from the GitHub API. This makes a network
/// request the first time and caches the result.
async fn zen_wisdom() -> &'static str {
    static ZEN: OnceCell<String> = OnceCell::const_new();
    ZEN.get_or_init(async || {
        let url = "https://api.github.com/zen";
        reqwest::get(url).await.unwrap().text().await.unwrap()
    })
    .await
}

#[tokio::main]
async fn main() {
    // Put some Zen wisdom in a stream.
    let mut zen_stream = pin!(stream::once(zen_wisdom()));
    // Start reading it, but cancel the read with a tight timeout.
    _ = timeout(Duration::from_millis(1), zen_stream.next()).await;
    // Now the stream is "snoozed", but it's still holding the `ZEN`
    // lock. Calling `zen_wisdom` again is a deadlock.
    println!("We make it here...");
    zen_wisdom().await;
    println!("...but not here.");
}
