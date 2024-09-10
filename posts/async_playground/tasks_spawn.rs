use std::collections::BTreeMap;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
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

type DynFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

static NEW_TASKS: Mutex<Vec<DynFuture>> = Mutex::new(Vec::new());

enum JoinState<T> {
    Unawaited,
    Awaited(Waker),
    Ready(T),
    Done,
}

struct JoinHandle<T> {
    state: Arc<Mutex<JoinState<T>>>,
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<T> {
        let mut guard = self.state.lock().unwrap();
        match mem::replace(&mut *guard, JoinState::Done) {
            JoinState::Ready(value) => Poll::Ready(value),
            JoinState::Unawaited | JoinState::Awaited(_) => {
                // Replace the previous Waker, if any. We only need the most recent one.
                *guard = JoinState::Awaited(context.waker().clone());
                Poll::Pending
            }
            JoinState::Done => unreachable!("polled again after Ready"),
        }
    }
}

fn spawn<F, T>(future: F) -> JoinHandle<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let join_state = Arc::new(Mutex::new(JoinState::Unawaited));
    let join_handle = JoinHandle {
        state: join_state.clone(),
    };
    let task = Box::pin(async move {
        let value = future.await;
        let mut guard = join_state.lock().unwrap();
        let previous = mem::replace(&mut *guard, JoinState::Ready(value));
        if let JoinState::Awaited(waker) = previous {
            waker.wake();
        }
    });
    NEW_TASKS.lock().unwrap().push(task);
    join_handle
}

// In production we'd use AtomicBool instead of Mutex<bool>.
struct AwakeFlag(Mutex<bool>);

impl AwakeFlag {
    fn is_awake(&self) -> bool {
        *self.0.lock().unwrap()
    }

    fn clear(&self) {
        *self.0.lock().unwrap() = false;
    }
}

impl Wake for AwakeFlag {
    fn wake(self: Arc<Self>) {
        *self.0.lock().unwrap() = true;
    }
}

async fn foo(n: u64) {
    println!("start {n}");
    sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}

async fn async_main() {
    let mut task_handles = Vec::new();
    for n in 1..=10 {
        task_handles.push(spawn(foo(n)));
    }
    for handle in task_handles {
        handle.await;
    }
}

fn main() {
    let awake_flag = Arc::new(AwakeFlag(Mutex::new(false)));
    let waker = Waker::from(Arc::clone(&awake_flag));
    let mut context = Context::from_waker(&waker);
    let mut main_task = Box::pin(async_main());
    let mut other_tasks: Vec<DynFuture> = Vec::new();
    loop {
        // Poll the main future and exit immediately if it's done.
        if main_task.as_mut().poll(&mut context).is_ready() {
            return;
        }
        // Poll other tasks and remove any that are Ready.
        let is_pending = |task: &mut DynFuture| task.as_mut().poll(&mut context).is_pending();
        other_tasks.retain_mut(is_pending);
        // Some tasks might have spawned new tasks. Pop from NEW_TASKS until it's empty. Note that
        // we can't use while-let here, because that would keep NEW_TASKS locked in the loop body.
        // See https://fasterthanli.me/articles/a-rust-match-made-in-hell.
        loop {
            let Some(mut task) = NEW_TASKS.lock().unwrap().pop() else {
                break;
            };
            // Poll each new task now, instead of waiting for the next iteration of the main loop,
            // to let them register wakeups. Drop the ones that return Ready. This poll can also
            // spawn more tasks, so it's important that NEW_TASKS isn't locked here.
            if task.as_mut().poll(&mut context).is_pending() {
                other_tasks.push(task);
            }
        }
        // Some tasks might wake other tasks. Re-poll if the AwakeFlag has been set. This might
        // poll futures that aren't ready yet, which is inefficient but allowed.
        if awake_flag.is_awake() {
            awake_flag.clear();
            continue;
        }
        // Sleep until the next Waker is scheduled and then invoke Wakers that are ready.
        let mut wakers_tree = WAKERS.lock().unwrap();
        if let Some(next_wake) = wakers_tree.keys().next() {
            thread::sleep(next_wake.saturating_duration_since(Instant::now()));
        }
        while let Some(entry) = wakers_tree.first_entry() {
            if *entry.key() <= Instant::now() {
                entry.remove().into_iter().for_each(Waker::wake);
            } else {
                break;
            }
        }
    }
}
