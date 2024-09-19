# Async Rust, Part One: Futures
###### \[date]

- [Introduction](async_intro.html)
- Part One: Futures (you are here)
  - [Foo](#foo)
  - [Join](#join)
  - [Sleep](#sleep)
  - [Wake](#wake)
  - [Main](#main)
  - [Aside: Pin](#aside__pin)
  - [Aside: Superpowers](#aside__superpowers)
- [Part Two: Tasks](async_tasks.html)
- [Part Three: IO](async_io.html)

In the introduction we looked at [some async Rust code][part_one] without
explaining anything about how it worked. That left us with several mysteries:
What's an `async fn`, and what are the "futures" that they return? What is
[`join_all`] doing? How is [`tokio::time::sleep`] different from
[`thread::sleep`]? What does `#[tokio::main]` actually do?

[part_one]: playground://async_playground/tokio.rs
[`join_all`]: https://docs.rs/futures/latest/futures/future/fn.join_all.html
[`tokio::time::sleep`]: https://docs.rs/tokio/latest/tokio/time/fn.sleep.html
[`thread::sleep`]: https://doc.rust-lang.org/std/thread/fn.sleep.html

I think the best way to answer these questions is to translate each piece into
normal, non-async Rust code and stare at it for a while. We'll find that we can
replicate `foo` and `join_all` without too much trouble, but writing our own
`sleep` is going to be a whole different story.[^universe] This will be the
most difficult part of this series, with the most new details that you need to
fit in your head at once. Here we go.

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
about to see a lot of this `Pin` business, and we have to talk about it. `Pin`
solves a big problem that async/await has in languages without a garbage
collector. It's a deep topic, and we'll touch on it again at the end of this
part, but _our_ code won't demonstrate the problem until much
later.[^iterators] So for now, I'm going to take an…unorthodox approach. I'm
gonna [just go on the internet and tell lies][lies].

[struct_pin]: https://doc.rust-lang.org/std/pin/struct.Pin.html

[^iterators]: Readers who already know about `Pin`: There was [a Tokio example
    in the introduction][tokio_serial] that implicitly relied on `Pin` for
    safety, because it held a local borrow across `.await` points. Can you spot
    it without clicking on the link? That fact didn't even occur to me when I
    wrote the example, which I think is a testament to the success of `Pin`.
    Our implementation will do something similar when we get to `JoinHandle` in
    Part Two.

[tokio_serial]: playground://async_playground/tokio_serial.rs

[lies]: https://www.youtube.com/watch?v=iHrZRJR4igQ&t=10s

`Pin` does nothing.[^truth] `Pin<Box<T>>` is the same as [`Box<T>`], which is
the same as `T`.[^box] We're about to see `.as_mut()`, which returns `Pin<&mut
T>`, which is the same as `&mut T`. Whenever you see `Pin`, please try to
ignore it.

[^truth]: As far as lies go, this one is pretty close to the truth. `Pin`'s job
    is to _prevent_ certain things in safe code, namely, moving certain futures
    after they've been polled. It's like how a shared reference prevents
    mutation. The reference doesn't _do_ much, but it represents permission.

[`Box<T>`]: https://doc.rust-lang.org/std/boxed/struct.Box.html

[^box]: This is a bigger lie, because unlike `Pin`, `Box<T>` actually _does_
    something: it puts `T` "on the heap". I'm using `Box::pin` as shortcut to
    avoid talking about ["pin projection"][projection]. But note that most
    futures in Rust are _not_ heap allocated, at least not individually. This
    is different from coroutines in C++20, which are automatically heap
    allocated.

[projection]: https://doc.rust-lang.org/std/pin/index.html#projections-and-structural-pinning

…

Seriously? Why put all these details on the page if we're just supposed to
ignore them? Because they're an unavoidable part of the [`Future`] trait, and
`Foo`'s whole purpose in life is to implement that trait. With all that in
mind, here's where the magic happens:

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

None of the calling code has changed. This non-async `fn foo` and the `struct
Foo` that it returns are drop-in replacements for `async fn foo`. They do
exactly the same thing. Click on the Playground button to see it run.

We finally have something more to say about what a "future" is. Apparently,
futures implement the `Future` trait and have a `poll` method. The `poll`
method asks, "Is this future finished?" If so, it returns [`Poll::Ready`] with
the future's `Output`.[^no_output] If not, it returns [`Poll::Pending`]. We can
see that `Foo::poll` won't return `Ready` until `Sleep::poll` has returned
`Ready`.

[`Poll::Ready`]: https://doc.rust-lang.org/std/task/enum.Poll.html
[`Poll::Pending`]: https://doc.rust-lang.org/std/task/enum.Poll.html

[^no_output]: `async fn foo` has no return value, so the `Foo` future has no
    `Output`. Rust represents no value with `()`, the empty tuple, also known
    as the "unit" type.

But `poll` isn't just a question; it's also where the work of the future
happens. In `Foo`'s case, it's where the printing happens. This forces a
compromise: `poll` does all the work that it can do right away, but as soon as
it needs to wait or block, it returns `Pending` instead.[^timing] The caller
gets its answer immediately, and in return it promises to call `poll` again
later. This compromise is the key to running thousands or millions of futures
at the same time, like we did in the introduction.

[^timing]: If you're skeptical, you can [add some timing and logging][timing]
    around `Sleep::poll` to see that it returns quickly.

[timing]: playground://async_playground/foo_timing.rs?mode=release

`Foo` doesn't know how many times it's going to be polled, and it shouldn't
print the "start" message more than once, so uses its `started` flag to keep
track.[^state_machine] This bookkeeping gets complicated when an `async fn` has
branches or loops, so it's nice that the compiler usually does it for
us.[^compiler] `Foo` doesn't need to track the "end" message, though, because
after it returns `Ready` it won't be polled again.[^iterator]

[^state_machine]: In other words `Foo` is a "state machine" with two states,
    plus whatever's inside of `Sleep`.

[^compiler]: Translating a loopy, branchy `async fn` into a state machine is
    kind of like translating a recursive algorithm into an iterative one. It's
    tricky and annoying for us humans but "easy" for a compiler. This is why
    async IO is usually a language feature and not just a library.

[^iterator]: Technically it's a "logic error" to call `poll` again after it's
    returned `Ready`. It could do anything, including blocking or panicking.
    But because `poll` isn't `unsafe`, it's not allowed to corrupt memory or
    commit other undefined behavior. This is similar to calling
    [`Iterator::next`] again after it's returned `None`.

[`Iterator::next`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#tymethod.next

We're starting to see what happened with the `thread::sleep` mistake at the end
of the introduction. If we use that blocking sleep in `Foo::poll` instead of
returning `Pending`, we get [exactly the same result][same_result]. We're
breaking the rule about `poll` returning quickly.

[same_result]: playground://async_playground/foo_blocking.rs

The last thing we have't talked about is the `Context` argument. For now, we
can see that `poll` receives it from above and passes it down when it polls
other futures. We'll have more to say shortly, when we implement our own
`sleep`.

Onward!

## Join

It might seem like [`join_all`] is doing something much more magical than
`foo`, but now that we've seen the moving parts of a future, it turns out we
already have everything we need. Let's make `join_all` into a non-async
function too:[^always_was]

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
argument, calls that closure on each element, and removes the elements that
returned `false`.[^algorithm] That means we drop each child future the first
time it returns `Ready`, following the rule that we're not supposed to `poll`
again after that.

[`Vec::retain_mut`]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.retain_mut

[^algorithm]: If we did this by calling `remove` in a loop, it would take
    O(n<sup>2</sup>) time, because `remove` is O(n). But `retain_mut` uses
    a clever algorithm that walks two pointers through the `Vec` and moves each
    element at most once.

[`Vec::remove`]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.remove

Having seen `Foo` above, there's nothing new here. From the outside, running
all these futures at the same time seemed like magic, but on the inside, all
we're doing is calling `poll` on the elements of a `Vec`. This is the other
side of the compromise: It works because `poll` returns quickly, and `poll`
knows that if it returns `Pending` we'll call it again later.

Note that we're taking a shortcut by ignoring the outputs of child
futures.[^payload] We're getting away with this because we only use our version
of `join_all` with `foo`, which has no output. The real `join_all` returns
`Vec<F::Output>`, which requires some more bookkeeping.

[^payload]: Specifically, when we call `.is_pending()` on the result of `poll`,
    we ignore any value that `Poll::Ready` might be carrying.

Onward!

## Sleep

We're on a roll here! It feels like we already have everything we need to
implement our own `sleep`:[^narrator]

[^narrator]: Narrator: They did not have everything they needed.

```rust
LINK: Playground playground://async_playground/sleep_forever.rs
struct Sleep {
    wake_time: Instant,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
        if Instant::now() >= self.wake_time {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

fn sleep(duration: Duration) -> Sleep {
    let wake_time = Instant::now() + duration;
    Sleep { wake_time }
}
```

(Playgroud running…)

Hmm. This compiles cleanly, and the logic in `poll` looks right, but running it
prints the "start" messages and then hangs forever. If we [add more
prints][sleep_forever_dbg], we can see that each `Sleep` gets polled once at
the start and then never again. What are we missing?

[sleep_forever_dbg]: playground://async_playground/sleep_forever_dbg.rs

It turns out that `poll` has [three jobs][poll_docs], and so far we've only
seen two. First, `poll` does as much work as it can without blocking. Check.
Second, `poll` returns `Ready` if its work is finished or `Pending` if there's
more work to do. Check. But third, whenever `poll` returns `Pending`, it needs
to "schedule a wakeup". Ah.

[poll_docs]: https://doc.rust-lang.org/std/future/trait.Future.html#tymethod.poll

The reason we didn't run into this before is that `Foo` and `JoinAll` only
return `Pending` when other futures return `Pending` to them, which means a
wakeup is already scheduled. But `Sleep` is what we call a "leaf" future. There
are no other futures below it,[^upside_down] and it needs to wake itself.

[^upside_down]: Trees in computing are upside down for some reason.

## Wake

It's finally time to make use of `poll`'s [`Context`] argument. If we call
`context.waker()`, we get something called a [`Waker`].[^only_method] Calling
either `waker.wake()` or `waker.wake_by_ref()` is how we ask for the current
future to be polled again.[^task]

[`Context`]: https://doc.rust-lang.org/std/task/struct.Context.html
[`Waker`]: https://doc.rust-lang.org/std/task/struct.Waker.html

[^only_method]: Handing out a `Waker` is currently all that `Context` does. An
    early version of the `Future` trait even had `poll` take a `Waker` directly
    instead of wrapping it in a `Context`, but the designers [wanted to leave
    open the possibility][possibility] of expanding the `poll` API in a
    backwards-compatible way. Note that `Waker` is `Clone` and `Send`, but
    `Context` is not.

[possibility]: https://github.com/rust-lang/rust/pull/59119

[^task]: Technically this wakes the current "task". We'll talk about tasks in
    Part Two.

The simplest thing we can try is immediately asking to be polled again every
time we return `Pending`:

```rust
LINK: Playground playground://async_playground/sleep_busy.rs
HIGHLIGHT: 5
fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
    if Instant::now() >= self.wake_time {
        Poll::Ready(())
    } else {
        context.waker().wake_by_ref();
        Poll::Pending
    }
}
```

This prints the right output and quits at the right time, so the "sleep
forever" problem is fixed, but we've replaced it with a "busy loop" problem.
This program calls `poll` over and over as fast as it can, burning 100% of the
CPU until the wake time. We can see this indirectly by [counting the number of
times `poll` gets called][sleep_busy_dbg],[^poll_count] or we can measure it
directly [using tools like `perf` on Linux][perf].

[sleep_busy_dbg]: playground://async_playground/sleep_busy_dbg.rs?mode=release

[^poll_count]: When I run this on the Playground, I see about 20&nbsp;_million_
    calls in total.

[perf]: https://github.com/oconnor663/jacko.io/blob/master/posts/async_playground/perf_stat.sh

We want to call `wake` later, when it's actually time to wake up. One way to do
that is to spawn a thread to call `thread::sleep` and `wake` for us. If we did
that in every call to `poll`, we'd run into the [too-many-threads crash from
the introduction][same_crash]. We could work around that by spawning one shared
thread and and [using a channel to send `Waker`s to it][shared_thread]. That
would be a correct and viable implementation, but there's something
unsatisfying about it&hellip;

We already have a thread that spends most of its time sleeping, the main thread
of our program! Why doesn't Tokio give us a way to tell the main thread to wake
up at a specific time, so that we don't need two sleeping threads? Well, there
is a way, that's what `tokio::time::sleep` _is_. But if we really want to write
our own `sleep`, and we don't want to spawn an extra thread to make it work,
then it turns out we also need to write our own `main`.

[same_crash]: playground://async_playground/sleep_many_threads.rs
[shared_thread]: playground://async_playground/sleep_one_thread.rs

## Main

Our `main` function wants to call `poll`, so it needs a `Context` to pass in.
We can make one with [`Context::from_waker`], which means we need a `Waker`.
There are a few different ways to make a `Waker`,[^make_a_waker] but since a
busy loop doesn't need it to do anything, we can use a helper function called
[`noop_waker`].[^noop] With a new `Context` in hand, we can call `poll` in a
loop:

[`Context::from_waker`]: https://doc.rust-lang.org/std/task/struct.Context.html#method.from_waker
[`noop_waker`]: https://docs.rs/futures/latest/futures/task/fn.noop_waker.html

[^make_a_waker]: The safe way is to implement the [`Wake`] trait and use
    [`Waker::from`][from_arc]. The unsafe way is to build a [`RawWaker`].

[`Wake`]: https://doc.rust-lang.org/alloc/task/trait.Wake.html
[from_arc]: https://doc.rust-lang.org/std/task/struct.Waker.html#impl-From%3CArc%3CW%3E%3E-for-Waker
[`RawWaker`]: https://doc.rust-lang.org/std/task/struct.RawWaker.html

[^noop]: "Noop", "no-op", and "nop" are all short for "no&nbsp;operation". Most
    assembly languages have an instruction named something like this, which
    does nothing.

```rust
LINK: Playground playground://async_playground/loop.rs
fn main() {
    let mut futures = Vec::new();
    for n in 1..=10 {
        futures.push(foo(n));
    }
    let mut joined_future = Box::pin(future::join_all(futures));
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    while joined_future.as_mut().poll(&mut context).is_pending() {
        // Busy loop!
    }
}
```

This works, but we still have the "busy loop" problem from above. Before we fix
that, though, we need to make another important mistake:

Since this version of our main loop[^event_loop] never stops polling, and since
our `Waker` does nothing, we might wonder whether calling `wake` in
`Sleep::poll` actually matters. Surprisingly, it does. If we delete it, [things
appear to work at first][loop_forever_10]. But when we bump the number of jobs
from ten to a hundred, [our futures never wake up][loop_forever_100]. What
we're seeing is that, even though _our_ `Waker` does nothing, there are _other_
`Waker`s hidden in our program. Specifically, when the real [`JoinAll`] has
many child futures,[^cutoff] it creates its own `Waker`s internally, which lets
it tell which child asked for a wakeup. That's more efficient than polling all
of them every time, but it means that children who invoke their own `Waker`
will never get polled again. Thus the rule is that `Pending` futures must
_always_ arrange to call `wake` somehow, even when they know main loop is
waking up anyway.

[^event_loop]: This is often called an "event loop", but right now all we have
    is sleeps, and those aren't really events. We'll build a proper event loop
    when we get to IO in Part Three. For now I'm going to call this the "main
    loop".

[loop_forever_10]: playground://async_playground/loop_forever_10.rs
[loop_forever_100]: playground://async_playground/loop_forever_100.rs
[`JoinAll`]: https://docs.rs/futures/latest/futures/future/fn.join_all.html

[^cutoff]: As of `futures` v0.3.30, [the exact cutoff is
    31](https://github.com/rust-lang/futures-rs/blob/0.3.30/futures-util/src/future/join_all.rs#L35).

Ok, back to `main`. Somehow our loop needs to get at each `Waker` and its wake
time. Let's use a global variable for this. As usual in Rust, we need to wrap
it in a `Mutex` if we want to mutate it from safe code:[^thread_local]

[^thread_local]: It would be slightly more efficient to [use `thread_local!`
    and `RefCell` instead of `Mutex`][thread_local], but `Mutex` is more
    familiar, and it's good enough.

[thread_local]: playground://async_playground/thread_local.rs

```rust
LINK: Playground playground://async_playground/wakers.rs
static WAKERS: Mutex<BTreeMap<Instant, Vec<Waker>>> =
    Mutex::new(BTreeMap::new());
```

This is a sorted map from wake times to `Waker`s.[^vec] We'll insert into this
map in `Sleep::poll`:[^or_default]

[^vec]: Note that the value type is `Vec<Waker>` instead of just `Waker`,
    because we might have more than one `Waker` for a given `Instant`. This is
    very unlikely on Linux and macOS, where the resolution of `Instant::now()`
    is measured in nanoseconds, but on Windows it's 15.6 ms.

[^or_default]: [`or_default`] creates an empty `Vec` if no value was there
    before.

[`or_default`]: https://doc.rust-lang.org/std/collections/btree_map/enum.Entry.html#method.or_default

```rust
LINK: Playground playground://async_playground/wakers.rs
HIGHLIGHT: 5-7
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
```

After polling, our main loop reads the first key from this sorted map to get
the earliest wake time. It `thread::sleep`s until that time, which fixes the
busy loop problem.[^hold_lock] Then it invokes all the `Waker`s whose wake time
has passed, before polling again:

[^hold_lock]: We're holding the `WAKERS` lock while we sleep here, which is a
    little sketchy, but it doesn't matter in this single-threaded example. A
    real multithreaded runtime would use [`thread::park_timeout`] or similar
    instead of sleeping, so that other threads could wake it up early.

[`thread::park_timeout`]: https://doc.rust-lang.org/std/thread/fn.park_timeout.html

```rust
LINK: Playground playground://async_playground/wakers.rs
HIGHLIGHT: 10-19
fn main() {
    let mut futures = Vec::new();
    for n in 1..=10 {
        futures.push(foo(n));
    }
    let mut joined_future = Box::pin(future::join_all(futures));
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    while joined_future.as_mut().poll(&mut context).is_pending() {
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
```

This works, and it does everything on one thread.

## Aside: Pin

Now that we have some intuition about how `async` functions turn into `Future`
structs, we can say a bit more about the problem that `Pin` solves. Imagine our
`async fn foo` took a reference internally for some reason:

```rust
LINK: Playground playground://async_playground/tokio_ref.rs
HIGHLIGHT: 2,3,5
async fn foo(n: u64) {
    let n_ref = &n;
    println!("start {n_ref}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n_ref}");
}
```

That compiles and runs just fine, and it looks like perfectly ordinary Rust
code. But what would the same change look like in our `Foo` future?

```rust
LINK: Playground playground://async_playground/compiler_errors/foo_ref.rs
HIGHLIGHT: 2,3
struct Foo {
    n: u64,
    n_ref: &u64,
    started: bool,
    sleep_future: Pin<Box<tokio::time::Sleep>>,
}
```

That doesn't compile:

```
LINK: Playground playground://async_playground/compiler_errors/foo_ref.rs
error[E0106]: missing lifetime specifier
 --> src/main.rs:3:12
  |
3 |     n_ref: &u64,
  |            ^ expected named lifetime parameter
```

What's the lifetime of `n_ref` supposed to be? The short answer is, there's no
good answer.[^longer] Self-referential borrows are generally illegal in Rust
structs, and there's no syntax for what `n_ref` is trying to do. Without this
rule, we'd have to ask tricky questions about when we're allowed to mutate `n`
and when we're allowed to move `Foo`.[^quote]

[^longer]: The longer answer is that we can hack a lifetime parameter onto
    `Foo`, but that makes it [impossible to do anything useful after we've
    constructed it][foo_ref_lifetime]. Unfortunately the compiler's hints in
    situations like this tend to be misleading, and "fighting the borrow
    checker" here takes us in circles.

[foo_ref_lifetime]: playground://async_playground/compiler_errors/foo_ref_lifetime.rs

[^quote]: "Most importantly, these objects are not meant to be _always
    immovable_. Instead, they are meant to be freely moved for a certain period
    of their lifecycle, and at a certain point they should stop being moved
    from then on. That way, you can move a self-referential future around as
    you compose it with other futures until eventually you put it into the
    place it will live for as long as you poll it. So we needed a way to
    express that an object is no longer allowed to be moved; in other words,
    that it is ‘pinned in place.'" - [without.boats/blog/pin][pin_post]

But then, how did we get away with `async fn foo` above? What `Future` struct
did the compiler generate for us?[^smart] It turns out that Rust does [some
very unsafe things][erase] internally to erase invalid
lifetimes.[^unsafe_pinned] The job of the `Pin` type is then to encapsulate all
that unsafety, so that we can write custom futures like `JoinAll` in safe code,
without the risk of dangling pointers or memory corruption.

[^smart]: A "sufficiently smart compiler" might optimize `n_ref` away in this
    simple case, but that won't work when we have complex iterators or when we
    share references with other futures.

[erase]: https://tmandry.gitlab.io/blog/posts/optimizing-await-1/#generators-as-data-structures

[^unsafe_pinned]: In fact, this transformation is [so wildly unsafe][transmute]
    that some of the compiler magic necessary to make it formally sound [hasn't
    been written yet][unsafe_pinned].

[transmute]: https://doc.rust-lang.org/nomicon/transmutes.html
[unsafe_pinned]: https://rust-lang.github.io/rfcs/3467-unsafe-pinned.html

We won't go any further into the details of the `Pin` API, but if you want the
whole story, start with [this post by the inventor of `Pin`][pin_post] and then
read through [the official `Pin` docs][pin_docs]. We're going to march off in a
different direction: tasks.

[pin_post]: https://without.boats/blog/pin
[pin_docs]: https://doc.rust-lang.org/std/pin

## Aside: Superpowers

TODO: cancellation and recursion

[a `timeout()` example][timeout]

[timeout]: playground://async_playground/timeout.rs

Regular recursion doesn't work:

```rust
LINK: Playground playground://async_playground/compiler_errors/recursion.rs
async fn factorial(n: u64) -> u64 {
    if n == 0 {
        1
    } else {
        n * factorial(n - 1).await
    }
}
```

We need to box the thing:


```rust
LINK: Playground playground://async_playground/boxed_recursion.rs
async fn factorial(n: u64) -> u64 {
    if n == 0 {
        1
    } else {
        let recurse = Box::pin(factorial(n - 1));
        n * recurse.await
    }
}
```

---

[← Introduction](async_intro.html) — [Part Two: Tasks →](async_tasks.html)
