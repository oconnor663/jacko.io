use std::time::Duration;

fn job(n: u64) {
    println!("start {n}");
    std::thread::sleep(Duration::from_secs(1));
    println!("end {n}");
}

fn main() {
    println!("Run three jobs, one at a time...\n");
    job(1);
    job(2);
    job(3);
}
