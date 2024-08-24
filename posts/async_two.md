# Async Rust, Part Two: How?
###### \[date]

- [Part One: Why?](async_one.html)
- Part Two: How? (you are here)
- [Part Three: More!](async_three.html)

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
replicate `foo` and `join_all` without too much trouble, but writing our own
`sleep` is going to be a whole different story.[^universe] Here we go.

[^universe]: [If you wish to make an apple pie from scratch, you must first
    invent the universe.](https://youtu.be/BkHCO8f2TWs?si=gIfadwLGsvawJ3qn)

## Foo

As a reminder, here's what `foo` looked like when it was an `async fn`:

```rust
LINK: Playground playground://async_playground/tokio_10.rs
async fn foo(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}
```

We can rewrite it as a regular, non-async function that returns a struct:

```rust
LINK: Playground playground://async_playground/foo.rs
fn foo(n: u64) -> Foo {
    let sleep_future = tokio::time::sleep(Duration::from_secs(1));
    Foo {
        n,
        started: false,
        sleep_future: Box::pin(sleep_future),
    }
}
```

This function calls [`tokio::time::sleep`], but it doesn't `.await` the future
that `sleep` returns.[^compiler_error] Instead, it stores it in a `Foo`
struct.[^convention] We do need to talk about [`Box::pin`], but let's look at
the struct first:

[^compiler_error]: It's a [compiler error] to use `.await` in a non-async
    function.

[compiler error]: playground://async_playground/compiler_errors/await.rs

[^convention]: It's conventional to use the same name lowercase for an async
    function and uppercase for the future it returns. So `foo` returns a `Foo`
    future, and `sleep` returns a [`Sleep`] future. This is similar to how
    [`zip`][zip_fn] returns a [`Zip`][zip_iter] iterator, and [`map`][map_fn]
    returns a [`Map`][map_iter] iterator. Futures and iterators have a lot in
    common.

[`Sleep`]: https://docs.rs/tokio/latest/tokio/time/struct.Sleep.html
[zip_fn]: https://doc.rust-lang.org/std/iter/fn.zip.html
[zip_iter]: https://doc.rust-lang.org/std/iter/struct.Zip.html
[map_fn]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.map
[map_iter]: https://doc.rust-lang.org/std/iter/struct.Map.html

[`Box::pin`]: https://doc.rust-lang.org/std/boxed/struct.Box.html#method.pin

```rust
LINK: Playground playground://async_playground/foo.rs
struct Foo {
    n: u64,
    started: bool,
    sleep_future: Pin<Box<tokio::time::Sleep>>,
}
```

Ok so apparently [`Box::pin`] returns a [`Pin::<Box<T>>`][struct_pin]. We're
about to see a lot of this `Pin` business, so I need to say something about it.
The whole story is big and fascinating and actually kind of
beautiful.[^beautiful] We'll come back to it in Part Three.[^whole_story] But
for now, I'm going to take an…unorthodox teaching strategy. I'm going to [just
go on the internet and tell lies][lies].

[struct_pin]: https://doc.rust-lang.org/std/pin/struct.Pin.html

[^beautiful]: “Most importantly, these objects are not meant to be _always
    immovable_. Instead, they are meant to be freely moved for a certain period
    of their lifecycle, and at a certain point they should stop being moved
    from then on. That way, you can move a self-referential future around as
    you compose it with other futures until eventually you put it into the
    place it will live for as long as you poll it. So we needed a way to
    express that an object is no longer allowed to be moved; in other words,
    that it is ‘pinned in place.’” - [without.boats/blog/pin][pin_post]

[^whole_story]: If you want the whole story right now, read [the post I just
    quoted][pin_post] from the inventor of `Pin`, and then read [the `std::pin`
    module docs][pin_docs].

[pin_docs]: https://doc.rust-lang.org/std/pin
[pin_post]: https://without.boats/blog/pin

[lies]: https://www.youtube.com/watch?v=iHrZRJR4igQ&t=10s

`Pin` does nothing.[^truth] `Pin<Box<T>>` is the same as [`Box<T>`], which is
the same as `T`.[^box] We're about to see `.as_mut()`, which returns `Pin<&mut
T>`, which is the same as `&mut T`. For the rest of Part Two, please try to
ignore `Pin`.

[^truth]: As far as lies go, this one is pretty close to the truth. `Pin`'s job
    is to _prevent_ certain things in safe code, namely, moving certain futures
    after they've been polled. It's like how a shared reference prevents
    mutation. The reference itself doesn't really _do_ anything; it just
    represents not having permission.

[`Box<T>`]: https://doc.rust-lang.org/std/boxed/struct.Box.html

[^box]: This is arguably a bigger lie, because unlike `Pin`, `Box<T>` actually
    _does_ something: it puts `T` "on the heap". I'm using `Box::pin` as
    shortcut to avoid talking about ["pin projection"][projection]. However,
    it's important to note that most futures in Rust are _not_ heap allocated,
    at least not individually. This is different from coroutines in C++20,
    which are automatically heap allocated.

[projection]: https://doc.rust-lang.org/std/pin/index.html#projections-and-structural-pinning

…

Seriously? Why put all these details on the page if we're just supposed to
ignore them? The reason is that they're an unavoidable part of the [`Future`]
trait, and `Foo`'s whole purpose in life is to implement that trait. So with
all that in mind, here's where the magic happens:

[`Future`]: https://doc.rust-lang.org/std/future/trait.Future.html

```rust
LINK: Playground playground://async_playground/foo.rs
impl Future for Foo {
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

Ok, we finally have something more to say about what a "future" is. A future
implements the `Future` trait and has a `poll` method. The `poll` method asks a
question: Is this future finished? If so, it returns [`Poll::Ready`] with the
future's `Output`.[^no_output] If not, it returns [`Poll::Pending`]. We can see
that `Foo::poll` won't return `Ready` until `Sleep::poll` has returned `Ready`.

[`Poll::Ready`]: https://doc.rust-lang.org/std/task/enum.Poll.html
[`Poll::Pending`]: https://doc.rust-lang.org/std/task/enum.Poll.html

[^no_output]: `async fn foo` has no return value, so the `Foo` future has no
    `Output`. Rust represents no value with `()`, the empty tuple, also known
    as the "unit" type. Functions and futures with no return value are used for
    their side effects, like printing.

But `poll` isn't just a question; it's also where the work of the future
happens. In our case, it's where the printing happens. This forces a
compromise: `poll` does all the work that it can do right away, but as soon as
it needs to wait or block, it returns `Pending` instead.[^timing] That way the
caller asking "Are you finished?" doesn't need to wait for an answer. In
return, the caller promises to call `poll` again later to let it finish its
work.

[^timing]: We can [add some timing and logging][timing] around the call to
    `Sleep::poll` to see that it always returns quickly too.

[timing]: playground://async_playground/foo_timing.rs?mode=release

`Foo::poll` doesn't know how many times it's going to be called, and it isn't
supposed to print the "start" message more than once, so uses its `started`
flag to keep track.[^state_machine] It doesn't need to track whether it's
printed the "end" message, though, because after it returns `Ready` it won't be
called again.[^iterator] This sort of bookkeeping can get quite complicated
when an `async fn` has branches or loops, so it's nice that the compiler
usually does all this for us.

[^state_machine]: In other words `Foo` is a "state machine" with two states,
    plus whatever's inside of `Sleep`.

[^iterator]: Technically it's a "logic error" to call `poll` again after it's
    returned `Ready`. It could do anything, including blocking or panicking.
    But because `poll` isn't `unsafe`, it's not allowed to corrupt memory or
    commit other undefined behavior. This is similar to calling
    [`Iterator::next`] again after it's returned `None`.

[`Iterator::next`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#tymethod.next

We're starting to see what happened with `std::thread::sleep` mistake at the
end of Part One. If we use that blocking sleep in `Foo::poll` instead of
returning `Pending`, we get [exactly the same result][same_result]. We're
breaking the rule about making the caller wait for an answer.

[same_result]: playground://async_playground/foo_blocking.rs

The last thing we have't talked about is the `Context` argument. For now, we
can see that `poll` receives it from above and passes it along whenever it
polls other futures. We'll look at it more closely when we implement our own
`sleep` below.

Onward!

## Join

It might seem like `join_all` is doing something much more magical than `foo`,
but now that we've seen the moving parts of a future, it turns out we already
have everything we need. Let's make `join_all` into a non-async function
too:[^always_was]

[^always_was]: In fact it's [defined this way upstream][upstream].

[upstream]: https://docs.rs/futures-util/0.3.30/src/futures_util/future/join_all.rs.html#102-105

```rust
LINK: Playground playground://async_playground/join.rs
struct JoinAll<F> {
    futures: Vec<Pin<Box<F>>>,
}

impl<F: Future> Future for JoinAll<F> {
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

fn join_all<F: Future>(futures: Vec<F>) -> JoinAll<F> {
    JoinAll {
        futures: futures.into_iter().map(Box::pin).collect(),
    }
}
```

[`Vec::retain_mut`] does all the heavy lifting here. It takes a closure
argument, calls that closure on each element of the `Vec`, and deletes the
elements that returned `false`.[^algorithm] That means we drop each child
future the first time it returns `Ready`, following the rule that we're not
supposed to `poll` again after that.

[`Vec::retain_mut`]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.retain_mut

[^algorithm]: If we did this by calling `remove` in a loop, it would take
    O(n<sup>2</sup>) time, because `remove` is O(n). But `retain_mut` uses
    a clever algorithm that walks two pointers through the `Vec` and moves each
    element at most once.

[`Vec::remove`]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.remove

Having seen `Foo` above, there's really nothing new here. From the outside, it
seemed like magic that we could run all these child futures at the same time,
but on the inside, all we're doing is calling `poll` on the elements of a
`Vec`. What makes this work is that each call to `poll` returns quickly, and
that when we return `Pending` ourselves, we get polled again later.

Note that we're taking a shortcut by ignoring the outputs of child
futures.[^payload] We're getting away with this because we only use our version
of `join_all` with `foo`, which has no output. The real `join_all` returns
`Vec<F::Output>`, which requires some more bookkeeping.

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
But if each future spawns a thread, we might run into [the same crash as in
Part One][same_crash]. [A single background thread that collects wakers through
a channel][background_thread] will work, but that's a bit complicated...

[same_crash]: playground://async_playground/sleep_many_threads.rs

[background_thread]: playground://async_playground/sleep_one_thread.rs

What we're seeing here is an important architectural fact about how async Rust
works. Futures "in the middle", like `Foo` and `JoinAll`, don't really need to
"know" anything about how the event loop works. But "leaf" futures like
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
        futures.push(foo(n));
    }
    let mut main_future = Box::pin(future::join_all(futures));
    let mut context = Context::from_waker(noop_waker_ref());
    while main_future.as_mut().poll(&mut context).is_pending() {
        // Busy loop!
    }
}
```

NOTE HERE: Even though our loop is always polling, we still need the wakers. If
we don't call `wake()`, our program will [appear to work at
first][loop_forever_10]. But then when we bump the number of jobs up to a
hundred, [it stops working][loop_forever_100].[^cutoff]

[loop_forever_10]: playground://async_playground/loop_forever_10.rs
[loop_forever_100]: playground://async_playground/loop_forever_100.rs

[^cutoff]: As of `futures` v0.3.30, the exact cutoff is
    [thirty-one](https://github.com/rust-lang/futures-rs/blob/0.3.30/futures-util/src/future/join_all.rs#L35).

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

And finally the main polling loop can read from it: [^hold_lock]

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
