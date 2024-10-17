use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

static WAKE_TIMES: Mutex<BTreeMap<Instant, Vec<Waker>>> =
    Mutex::new(BTreeMap::new());

fn sleep(duration: Duration) -> Sleep {
    let wake_time = Instant::now() + duration;
    Sleep { wake_time }
}

struct Sleep {
    wake_time: Instant,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if Instant::now() >= self.wake_time {
            Poll::Ready(())
        } else {
            let mut wake_times = WAKE_TIMES.lock().unwrap();
            let wakers_vec = wake_times.entry(self.wake_time).or_default();
            wakers_vec.push(context.waker().clone());
            Poll::Pending
        }
    }
}

type DynFuture = Pin<Box<dyn Future<Output = ()>>>;

async fn foo(n: u64) {
    println!("start {n}");
    sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}

fn main() {
    let mut tasks: Vec<DynFuture> = Vec::new();
    for n in 1..=10 {
        tasks.push(Box::pin(foo(n)));
    }
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    loop {
        // Poll each task and remove any that are Ready.
        let is_pending = |task: &mut DynFuture| {
            task.as_mut().poll(&mut context).is_pending()
        };
        tasks.retain_mut(is_pending);
        // If there are no tasks left, we're done. Note that this is different from Tokio.
        if tasks.is_empty() {
            break;
        }
        // Sleep until the next Waker is scheduled and then invoke Wakers that are ready.
        let mut wake_times = WAKE_TIMES.lock().unwrap();
        let next_wake = wake_times.keys().next().expect("sleep forever?");
        thread::sleep(next_wake.saturating_duration_since(Instant::now()));
        while let Some(entry) = wake_times.first_entry() {
            if *entry.key() <= Instant::now() {
                entry.remove().into_iter().for_each(Waker::wake);
            } else {
                break;
            }
        }
    }
}
