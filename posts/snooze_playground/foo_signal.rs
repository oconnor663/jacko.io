use std::thread;
use std::time::Duration;

static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn foo() {
    let _guard = LOCK.lock().unwrap();
    // Use a longer sleep in this example, to account for child process
    // spawning time.
    thread::sleep(Duration::from_millis(1000));
}

extern "C" fn foo_signal_handler(_signum: libc::c_int) {
    // Printing in signal handlers isn't really allowed, but neither is
    // taking locks, in part because of exactly the sort of deadlocks we're
    // demonstrating here. It works in this case.
    println!("We make it here...");
    foo();
    println!("...but not here!");
}

fn main() -> std::io::Result<()> {
    // Set a signal handler for SIGUSR1 that calls foo().
    unsafe {
        libc::signal(libc::SIGUSR1, foo_signal_handler as *const () as _);
    }
    // Spawn a child process that sends us SIGUSR1 after 500 ms.
    std::process::Command::new("bash")
        .arg("-c")
        .arg(format!("sleep 0.5 && kill -USR1 {}", std::process::id()))
        .spawn()?;
    // Call `foo` ourselves. We'll be in this call and holding `LOCK` in
    // ~500 ms when SIGUSR1 arrives and the signal handler fires. It'll
    // hijack this thread, try to take `LOCK` again, and deadlock.
    foo();
    Ok(())
}
