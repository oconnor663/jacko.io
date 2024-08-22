use std::time::Duration;

fn foo(n: u64) {
    println!("start {n}");
    std::thread::sleep(Duration::from_secs(1));
    println!("end {n}");
}

fn main() {
    println!("Run three jobs, one at a time...\n");
    foo(1);
    foo(2);
    foo(3);
}
