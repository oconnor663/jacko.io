use std::time::Duration;

fn main() {
    tokio::time::sleep(Duration::from_secs(1)).await;
}
