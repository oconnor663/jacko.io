use std::time::Duration;

async fn foo(n: u64) {
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("finished foo {n}");
}

#[tokio::main]
async fn main() {
    println!("Spawn 10 tasks in 2 seconds and wait for all of them to finish.\n");
    let mut task_handles = Vec::new();
    for n in 1..=10 {
        task_handles.push(tokio::task::spawn(foo(n)));
        println!("started foo {n}");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    for handle in task_handles {
        handle.await.unwrap();
    }
}
