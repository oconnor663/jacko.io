use std::time::Duration;

fn job(n: u64) {
    println!("start {n}");
    std::thread::sleep(Duration::from_secs(1));
    println!("end {n}");
}

fn main() {
    println!("Run three jobs at the same time...\n");
    let mut threads = Vec::new();
    for n in 1..=3 {
        threads.push(std::thread::spawn(move || job(n)));
    }
    for thread in threads {
        thread.join().unwrap();
    }
}
