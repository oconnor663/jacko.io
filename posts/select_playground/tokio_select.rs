use tokio::select;
use tokio::time::{Duration, sleep};

async fn print_sleep(name: &str, sleep_ms: u64) -> &str {
    println!("sleep {name} started ({sleep_ms} ms)");
    sleep(Duration::from_millis(sleep_ms)).await;
    println!("sleep {name} finished");
    name
}

#[tokio::main]
async fn main() {
    // It's not really a mystery who's going to win this race...
    let a = print_sleep("A", 1_000);
    let b = print_sleep("B", 2_000);
    select! {
        _ = a => println!("A won!"),
        _ = b => println!("B won!"),
    };
}
