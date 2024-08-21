use futures::task::noop_waker_ref;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

static TASK_SENDER: OnceLock<Sender<BoxedFuture>> = OnceLock::new();
static WAKERS: Mutex<BTreeMap<Instant, Vec<Waker>>> = Mutex::new(BTreeMap::new());

type BoxedFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

struct SleepFuture {
    wake_time: Instant,
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if self.wake_time <= Instant::now() {
            Poll::Ready(())
        } else {
            let mut wakers_tree = WAKERS.lock().unwrap();
            let wakers_vec = wakers_tree.entry(self.wake_time).or_default();
            wakers_vec.push(context.waker().clone());
            Poll::Pending
        }
    }
}

fn sleep(duration: Duration) -> SleepFuture {
    let wake_time = Instant::now() + duration;
    SleepFuture { wake_time }
}

fn spawn_task<F: Future<Output = ()> + Send + 'static>(future: F) {
    TASK_SENDER.get().unwrap().send(Box::pin(future)).unwrap();
}

async fn job(n: u64) {
    sleep(Duration::from_secs(1)).await;
    println!("finished job {n}");
}

async fn async_main() {
    println!("Spawn 10 tasks in 2 seconds and wait for all of them to finish.\n");
    for n in 1..=10 {
        spawn_task(job(n));
        println!("started job {n}");
        sleep(Duration::from_millis(200)).await;
    }
    // NOTE: Tokio exits when the main task is finished, so we need to collect task
    // handles and await them. Our implementation below continues until all tasks
    // are finished, so we don't need to collect (or implement) task handles.
}

fn main() {
    let (task_sender, task_receiver) = channel();
    TASK_SENDER.set(task_sender).unwrap();
    let mut context = Context::from_waker(noop_waker_ref());
    println!("Start with one task (many_jobs), which spawns");
    println!("ten other tasks (job) in two seconds.\n");
    let mut tasks: Vec<BoxedFuture> = vec![Box::pin(async_main())];
    loop {
        // Poll all existing tasks, removing any that are finished.
        let is_pending = |task: &mut BoxedFuture| task.as_mut().poll(&mut context).is_pending();
        tasks.retain_mut(is_pending);
        // Any of the tasks we just polled might've called spawn_task() internally. Drain the
        // TASK_SENDER channel into our local tasks Vec.
        while let Ok(mut task) = task_receiver.try_recv() {
            // Poll each new tasks once, and keep the ones that are pending.
            if task.as_mut().poll(&mut context).is_pending() {
                tasks.push(task);
            }
        }
        // If there are no tasks left, we're done!
        if tasks.is_empty() {
            break;
        }
        // Sleep until the next Waker is scheduled and then invoke Wakers that are ready.
        let mut wakers_tree = WAKERS.lock().unwrap();
        let next_wake = wakers_tree.keys().next().expect("sleep forever?");
        std::thread::sleep(next_wake.duration_since(Instant::now()));
        while let Some(entry) = wakers_tree.first_entry() {
            if *entry.key() <= Instant::now() {
                entry.remove().into_iter().for_each(Waker::wake);
            } else {
                break;
            }
        }
    }
}
