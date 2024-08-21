# Async Rust, Part Two: How does it work?
###### \[date]

- [Part One: What's in it for us?](async_one.html)
- Part Two: How does it work? (you are here)
- [Part Three: Choose your own adventure](async_three.html)

In Part One we looked at [some async Rust code][part_one] without explaining
anything about how it worked. That left us with several mysteries: What's an
`async fn`, and what are the "futures" that they return? What is [`join_all`]
doing? How is [`tokio::time::sleep`] different from [`std::thread::sleep`]?
What does `#[tokio::main]` actually do?

[part_one]: playground://async_playground/tokio.rs
[`join_all`]: https://docs.rs/futures/latest/futures/future/fn.join_all.html
[`tokio::time::sleep`]: https://docs.rs/tokio/latest/tokio/time/fn.sleep.html
[`std::thread::sleep`]: https://doc.rust-lang.org/std/thread/fn.sleep.html

I think the best way to answer these questions is to translate each piece into
normal, non-async Rust code and stare at it for a while. We'll find that we can
replicate `job` and `join_all` without too much trouble, but writing our own
`sleep` is going to be a whole different story.[^universe] Here we go.

[^universe]: [If you wish to make an apple pie from scratch, you must first
    invent the universe.](https://youtu.be/BkHCO8f2TWs?si=gIfadwLGsvawJ3qn)

## Job

As a reminder, here's what `job` looked like when it was an `async fn`:

```rust
LINK: Playground playground://async_playground/tokio.rs
async fn job(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}
```

We can rewrite it as a regular, non-async function that returns a future:

```rust
LINK: Playground playground://async_playground/job.rs
fn job(n: u64) -> JobFuture {
    let sleep_future = tokio::time::sleep(Duration::from_secs(1));
    JobFuture {
        n,
        started: false,
        sleep_future: Box::pin(sleep_future),
    }
}
```

You might want to open both versions on the Playground and look at them side by
side. Notice that the non-async version calls `tokio::time::sleep` but doesn't
`.await`[^compiler_error] the [`Sleep`] future that `sleep`
returns.[^uppercase] Instead it stores the `Sleep` future in a new
struct.[^box_pin] Here's the struct:

[^compiler_error]: It's a [compiler error] to use `.await` in a non-async
    function.

[compiler error]: playground://async_playground/compiler_errors/error.rs

[`Sleep`]: https://docs.rs/tokio/latest/tokio/time/struct.Sleep.html

[^uppercase]: To repeat, `sleep` (lowercase) is an async function and `Sleep`
    (uppercase) is the future that it returns. It's confusing, but it's similar
    to how the [`map`] method on iterators returns an iterator called [`Map`].
    Futures and iterators have a lot in common, as we'll see.

[`map`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.map
[`Map`]: https://doc.rust-lang.org/std/iter/struct.Map.html

[^box_pin]: Wait a minute, what's `Box::pin`? Hold that thought for just a moment.

```rust
LINK: Playground playground://async_playground/job.rs
struct JobFuture {
    n: u64,
    started: bool,
    sleep_future: Pin<Box<tokio::time::Sleep>>,
}

impl Future for JobFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if !self.started {
            println!("start {}", self.n);
            self.started = true;
        }
        if self.sleep_future.as_mut().poll(context).is_pending() {
            Poll::Pending
        } else {
            println!("end {}", self.n);
            Poll::Ready(())
        }
    }
}
```

This is a lot to take in all at once. Before we even get started, I want to set
aside a couple things that we're not going to explain until later. The first is
the `Context` argument. We'll look at that below when we implement `sleep`. The
second is `Pin`. We'll come back to `Pin` in Part Three, but for now if you'll
forgive me, I'm going to bend the truth a little bit: `Pin` doesn't do
anything.[^lies] Think of `Pin<Box<T>>` as `Box<T>`,[^box] think of `Pin<&mut
T>` as a `&mut T`, and try not to think about `as_mut` at all.

[^lies]: As far as lies go, this one is surprisingly close to the truth.

[^box]: And if you haven't seen [`Box<T>`][box] before, that's just `T` "on the
    heap". The difference between the "stack" and the "heap" is an important
    part of systems programming, but for now we're skipping over all the
    details that aren't absolutely necessary. They'll be easier to remember
    once you know how the story ends.

[box]: https://doc.rust-lang.org/std/boxed/struct.Box.html

Ok, with those caveats out of the way, let's get into some details. We finally
have something more to say about what a "future" is. It's something that
implements the [`Future`] trait. Our `JobFuture` implements `Future`, so has a
`poll` method. The `poll` method asks a question: Is the future finished with
its work? If so, `poll` returns [`Poll::Ready`] with its `Output`.[^no_output]
If not, `poll` returns [`Poll::Pending`]. We can see that `JobFuture::poll`
won't return `Ready` until [`Sleep::poll`][Sleep] has returned `Ready`.

[`Future`]: https://doc.rust-lang.org/std/future/trait.Future.html
[`Poll::Ready`]: https://doc.rust-lang.org/std/task/enum.Poll.html
[`Poll::Pending`]: https://doc.rust-lang.org/std/task/enum.Poll.html
[Sleep]: https://docs.rs/tokio/latest/tokio/time/struct.Sleep.html

[^no_output]: Our original `job` function had no return value, so `JobFuture`
    has no `Output`. Rust represents no value with `()`, the empty tuple, also
    known as the "unit" type. Functions and futures with no return value are
    used for their side effects, like printing.

But `poll` isn't just a question. It's also where the work of the future
happens. When it's time for `job` to print, it's `JobFuture::poll` that does
the printing. So there's a compromise: `poll` does as much work as it can get
done quickly, but whenever it would need to wait or block, it returns `Pending`
instead.[^timing] That way the caller that's asking "Are you finished?" never
needs to wait for an answer. In return, the caller promises to call `poll`
again later to let it finish its work.

[^timing]: We can [add some timing and logging][timing] around the call to
    `Sleep::poll` to see that it always returns quickly too.

[timing]: playground://async_playground/job_timing.rs?mode=release

`JobFuture::poll` doesn't know how many times it's going to be called, and it
shouldn't print the "start" message more than once, so sets its `started` flag
to keep track.[^state_machine] It doesn't need to track whether it's printed
the "end" message, though, because after it returns `Ready` it won't be called
again.[^iterator]

[^state_machine]: In other words, `JobFuture` is a "state machine" with two
    states. In general, the number of states you need to track where you are in
    an `async fn` is the number of `.await` points plus one, but this gets
    complicated when there are branches or loops. The magic of async is that
    the compiler figures all this out for us, and we don't usually need to
    write our own `poll` functions like we're doing here.

[^iterator]: Technically it's a "logic error" to call `poll` again after it's
    returned `Ready`. It could do anything, including blocking or panicking.
    But because `poll` is not `unsafe`, it's not allowed to corrupt memory or
    commit other undefined behavior. It's exactly the same story as calling
    [`Iterator::next`] again after it's returned `None`.

[`Iterator::next`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#tymethod.next

We're starting to see how `std::thread::sleep` ruined our performance at the
end of Part One. If we put a blocking sleep in `JobFuture::poll` instead of
returning `Pending`, we get [exactly the same result][same_result].

[same_result]: playground://async_playground/job_blocking.rs

Onward!

## Join

It might seem like `join_all` is doing something much more magical than `job`,
but now that we've seen the moving parts of a future, it turns out we already
have everything we need. Let's make `join_all` into a non-async function
too:[^always_was]

[^always_was]: In fact it's [defined this way upstream][upstream].

[upstream]: https://docs.rs/futures-util/0.3.30/src/futures_util/future/join_all.rs.html#102-105

```rust
LINK: Playground playground://async_playground/join.rs
fn join_all<F: Future>(futures: Vec<F>) -> JoinFuture<F> {
    JoinFuture {
        futures: futures.into_iter().map(Box::pin).collect(),
    }
}
```

Once again, the function doesn't do much,[^agreement] and all the interesting
work happens in the struct:

[^agreement]: Especially since we've agreed not to think too hard about `Box::pin`.

```rust
LINK: Playground playground://async_playground/join.rs
struct JoinFuture<F> {
    futures: Vec<Pin<Box<F>>>,
}

impl<F: Future> Future for JoinFuture<F> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        let is_pending = |future: &mut Pin<Box<F>>| {
            future.as_mut().poll(context).is_pending()
        };
        self.futures.retain_mut(is_pending);
        if self.futures.is_empty() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
```

[`Vec::retain_mut`] does most of the heavy lifting. It takes a closure
argument, calls that closure on each element of the `Vec`, and deletes the
elements that returned `false`.[^algorithm] Here that means that we drop each
child future the first time it returns `Ready`, following the rule that we're
not supposed to `poll` them again after that.

[`Vec::retain_mut`]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.retain_mut

[^algorithm]: If we did this with a simple `for` loop, it would take
    O(n<sup>2</sup>) time, because `Vec::remove` is O(n). But `retain_mut` uses
    a clever algorithm that walks two pointers through the `Vec` and moves each
    element at most once.

[`Vec::remove`]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.remove

Having seen `JobFuture` above, there's really nothing else new here. From the
outside, it feels magical that we can run all these child futures at once, but
on the inside, all we're doing is calling `poll` on the elements of a `Vec`.
What makes this work is that each call to `poll` returns quickly, and that when
we return `Pending` we get called again later.

Note that we're taking a shortcut by ignoring the outputs of child
futures.[^payload] We can get away with that because we only use our version of
`join_all` with `job`, which has no return value. The real `join_all` returns a
`Vec<F::Output>`, and it need to do some more bookkeeping.

[^payload]: Specifically, when we call `.is_pending()` on the result of `poll`,
    we ignore any value that `Poll::Ready` might be carrying.

Onward!

## Sleep

This version never wakes up:

```rust
LINK: Playground playground://async_playground/sleep_forever.rs
struct SleepFuture {
    wake_time: Instant,
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
        if self.wake_time <= Instant::now() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

fn sleep(duration: Duration) -> SleepFuture {
    let wake_time = Instant::now() + duration;
    SleepFuture { wake_time }
}
```

## Wake

This version always wakes up, so the output is correct, but it burns the CPU:

```rust
LINK: Playground playground://async_playground/sleep_busy.rs
fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
    if self.wake_time <= Instant::now() {
        Poll::Ready(())
    } else {
        context.waker().wake_by_ref();
        Poll::Pending
    }
}
```

The simplest way to avoid a busy wait is to spawn a thread to wake us up later.
If [each future spawned its own thread][same_crash], we'd run into the same
crash as in Part One. [A single background thread that collects wakers through
a channel][background_thread] will work, but that's a bit complicated...

[same_crash]: playground://async_playground/sleep_many_threads.rs

[background_thread]: playground://async_playground/sleep_one_thread.rs

What we're seeing here is an important architectural fact about how async Rust
works. Futures "in the middle", like `JobFuture` and `JoinFuture`, don't really
need to "know" anything about how the event loop works. But "leaf" futures like
`SleepFuture` need to coordinate closely with the event loop to schedule
wakeups. This is why writing runtime-agnostic async libraries is hard.

## Loop

It's more interesting to get the event loop to wake up at the right time. To do
that we need to rewrite it. Here's the minimal custom event loop:

```rust
LINK: Playground playground://async_playground/loop.rs
fn main() {
    let mut futures = Vec::new();
    for n in 1..=1_000 {
        futures.push(job(n));
    }
    let mut main_future = Box::pin(future::join_all(futures));
    let mut context = Context::from_waker(noop_waker_ref());
    while main_future.as_mut().poll(&mut context).is_pending() {
        // Busy loop!
    }
}
```

NOTE HERE: Even though our loop is always polling, we still need the wakers. If
we don't call `wake()` our program never finishes.

Now instead of busy looping, we can tell that loop how long to sleep. Let's add
a global:[^thread_local]

[^thread_local]: It would be slightly more efficient to [use `thread_local!`
    and `RefCell` instead of `Mutex`][thread_local], but `Mutex` is the
    familiar way to make a global variable in safe Rust, and it's good enough.

[thread_local]: playground://async_playground/thread_local.rs

```rust
static WAKERS: Mutex<BTreeMap<Instant, Vec<Waker>>> =
    Mutex::new(BTreeMap::new());
```

And have `SleepFuture` put wakers in there:

```rust
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
```

And finally the main polling loop can read from it:[^instant_only] [^hold_lock]

[^instant_only]: You might wonder why we bother calling `wake` here. Our
    top-level `Waker` is a no-op, we've already finished sleeping, and we're
    about to poll again, so what's the point? Well, it turns out that fancy
    combinators like [`JoinAll`] (not our simple version above, but the real
    one from [`futures`]) create a unique `Waker` internally for each of their
    children, and [they only poll children that have been awakened][skip_wake].
    This sort of thing is why [the docs for `Poll::Pending`][contract] say we
    must eventually wake the "current task".

[`JoinAll`]: https://docs.rs/futures/latest/futures/future/fn.join_all.html
[`futures`]: https://docs.rs/futures
[contract]: https://doc.rust-lang.org/std/task/enum.Poll.html#variant.Pending

[skip_wake]: playground://async_playground/skip_wakers.rs

[^hold_lock]: We're holding the `WAKERS` lock while we sleep here, which is a
    little sketchy, but it doesn't matter in this single-threaded example. A
    real multithreaded runtime would use [`std::thread::park_timeout`] or
    similar instead of sleeping, so that other threads could wake it up early.

[`std::thread::park_timeout`]: https://doc.rust-lang.org/std/thread/fn.park_timeout.html

```rust
LINK: Playground playground://async_playground/wakers.rs
while main_future.as_mut().poll(&mut context).is_pending() {
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
```
