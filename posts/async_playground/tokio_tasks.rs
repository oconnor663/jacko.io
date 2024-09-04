use std::time::Duration;

async fn foo(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}

#[tokio::main]
async fn main() {
    let mut task_handles = Vec::new();
    for n in 1..=10 {
        task_handles.push(tokio::task::spawn(foo(n)));
    }
    for handle in task_handles {
        handle.await.unwrap();
    }
}
