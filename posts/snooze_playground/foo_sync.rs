use std::thread;
use std::time::Duration;

static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn foo() {
    let _guard = LOCK.lock().unwrap();
    thread::sleep(Duration::from_millis(10));
}

fn main() {
    foo();
    foo();
    foo();
}
