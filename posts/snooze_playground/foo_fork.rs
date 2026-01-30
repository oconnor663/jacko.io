use std::os::unix::process::CommandExt;
use std::thread;
use std::time::Duration;

static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn foo() {
    let _guard = LOCK.lock().unwrap();
    // Use a longer sleep in this example, to account for child process
    // spawning time.
    thread::sleep(Duration::from_millis(1000));
}

fn main() -> std::io::Result<()> {
    // Spawn a background thread that calls foo().
    thread::spawn(foo);
    // Sleep to give the background thread time to take `LOCK`.
    thread::sleep(Duration::from_millis(500));
    // Fork a child process while `LOCK` is held.
    let mut command = std::process::Command::new("echo");
    unsafe {
        command.pre_exec(|| {
            // This closure runs post-fork-pre-exec in the child.
            println!("We make it here...");
            foo();
            println!("...but not here!");
            Ok(())
        });
    };
    command.status()?;
    Ok(())
}
