use std::thread;
use std::time::Duration;

fn foo(n: u64) {
    println!("start {n}");
    thread::sleep(Duration::from_secs(1));
    println!("end {n}");
}

fn main() {
    let mut threads = Vec::new();
    for n in 1..=100 {
        threads.push(thread::spawn(move || foo(n)));
    }
    for thread in threads {
        thread.join().unwrap();
    }
}
