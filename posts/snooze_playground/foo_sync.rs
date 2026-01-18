use std::time::Duration;

static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn foo() {
    let _guard = LOCK.lock().unwrap();
    std::thread::sleep(Duration::from_millis(10));
}

fn main() {
    foo();
    foo();
    foo();
}
