async fn factorial(n: u64) -> u64 {
    if n == 0 {
        1
    } else {
        n * factorial(n - 1).await
    }
}

#[tokio::main]
async fn main() {
    println!("{}", factorial(10).await);
}
