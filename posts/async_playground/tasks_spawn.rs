use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

static TASK_SENDER: OnceLock<Sender<DynFuture>> = OnceLock::new();
static WAKERS: Mutex<BTreeMap<Instant, Vec<Waker>>> = Mutex::new(BTreeMap::new());

type DynFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

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

fn spawn<F: Future<Output = ()> + Send + 'static>(future: F) {
    TASK_SENDER.get().unwrap().send(Box::pin(future)).unwrap();
}

async fn foo(n: u64) {
    println!("start {n}");
    sleep(Duration::from_secs(1)).await;
    println!("end {n}");
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
    let (task_sender, task_receiver) = channel();
    TASK_SENDER.set(task_sender).unwrap();
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    let mut tasks: Vec<DynFuture> = vec![Box::pin(async_main())];
    loop {
        // Poll each task, removing any that are Ready.
        let is_pending = |task: &mut DynFuture| task.as_mut().poll(&mut context).is_pending();
        tasks.retain_mut(is_pending);
        // Any of the tasks we just polled might've called spawn_task() internally. Drain the
        // TASK_SENDER channel into our local tasks Vec.
        while let Ok(mut task) = task_receiver.try_recv() {
            // Poll each new tasks once, and keep the ones that are pending.
            if task.as_mut().poll(&mut context).is_pending() {
                tasks.push(task);
            }
        }
        // If there are no tasks left, we're done.
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
