use std::time::Duration;

async fn foo(n: u64) {
    let n_ref = &n;
    println!("start {n_ref}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n_ref}");
}

#[tokio::main]
async fn main() {
    foo(1).await;
    foo(2).await;
    foo(3).await;
}
