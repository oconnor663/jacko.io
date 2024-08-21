async fn factorial(n: u64) -> u64 {
    if n == 0 {
        1
    } else {
        let recurse = Box::pin(factorial(n - 1));
        n * recurse.await
    }
}

#[tokio::main]
async fn main() {
    println!("{}", factorial(10).await);
}
