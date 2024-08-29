use std::thread;
use std::time::Duration;

fn foo(n: u64) {
    println!("start {n}");
    thread::sleep(Duration::from_secs(1));
    println!("end {n}");
}

fn main() {
    println!("Run a thousand jobs on a thread pool...");
    rayon::scope(|scope| {
        for n in 1..=1_000 {
            scope.spawn(move |_| foo(n));
        }
        println!("All the jobs have been spawned...");
    });
}
