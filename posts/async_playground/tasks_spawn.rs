use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

static WAKERS: Mutex<BTreeMap<Instant, Vec<Waker>>> = Mutex::new(BTreeMap::new());

struct Sleep {
    wake_time: Instant,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if Instant::now() >= self.wake_time {
            Poll::Ready(())
        } else {
            let mut wakers_tree = WAKERS.lock().unwrap();
            let wakers_vec = wakers_tree.entry(self.wake_time).or_default();
            wakers_vec.push(context.waker().clone());
            Poll::Pending
        }
    }
}

fn sleep(duration: Duration) -> Sleep {
    let wake_time = Instant::now() + duration;
    Sleep { wake_time }
}

async fn foo(n: u64) {
    println!("start {n}");
    sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}

type DynFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

static NEW_TASKS: Mutex<Vec<DynFuture>> = Mutex::new(Vec::new());

fn spawn<F: Future<Output = ()> + Send + 'static>(future: F) {
    NEW_TASKS.lock().unwrap().push(Box::pin(future));
}

async fn async_main() {
    // Tokio exits when the main task is finished, like Rust exits when the main thread is
    // finished. So with tokio::task::spawn, we needed to collect task handles and explicitly await
    // them. But for simplicity we've skipped implementing task handles, and our main loop runs
    // until all tasks are finished, so we can spawn() without collecting anything.
    for n in 1..=10 {
        spawn(foo(n));
    }
}

fn main() {
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    let mut tasks: Vec<DynFuture> = vec![Box::pin(async_main())];
    loop {
        // Poll each task, removing any that are Ready.
        let is_pending = |task: &mut DynFuture| task.as_mut().poll(&mut context).is_pending();
        tasks.retain_mut(is_pending);
        // The tasks we just polled might've spawned new tasks. Pop from NEW_TASKS until it's
        // empty. Note that we can't use while-let here, because that would keep NEW_TASKS locked
        // in the loop body. See https://fasterthanli.me/articles/a-rust-match-made-in-hell.
        loop {
            let Some(mut task) = NEW_TASKS.lock().unwrap().pop() else {
                break;
            };
            // Poll each new task now, instead of waiting for the next iteration of the main loop,
            // to let them register wakeups. Drop the ones that return Ready. This poll can also
            // spawn more tasks, so it's important that NEW_TASKS isn't locked here.
            if task.as_mut().poll(&mut context).is_pending() {
                tasks.push(task);
            }
        }
        // If there are no tasks left, we're done. Note that this is different from Tokio.
        if tasks.is_empty() {
            break;
        }
        // Sleep until the next Waker is scheduled and then invoke Wakers that are ready.
        let mut wakers_tree = WAKERS.lock().unwrap();
        let next_wake = wakers_tree.keys().next().expect("sleep forever?");
        thread::sleep(next_wake.saturating_duration_since(Instant::now()));
        while let Some(entry) = wakers_tree.first_entry() {
            if *entry.key() <= Instant::now() {
                entry.remove().into_iter().for_each(Waker::wake);
            } else {
                break;
            }
        }
    }
}
