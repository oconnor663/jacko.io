# Async Rust, Part One: Futures
###### \[DRAFT]

- [Introduction](async_intro.html)
- Part One: Futures (you are here)
  - [Foo](#foo)
  - [Join](#join)
  - [Sleep](#sleep)
  - [Wake](#wake)
  - [Main](#main)
  - [Bonus: Pin](#bonus__pin)
  - [Bonus: Cancellation](#bonus__cancellation)
  - [Bonus: Recursion](#bonus__recursion)
- [Part Two: Tasks](async_tasks.html)
- [Part Three: IO](async_io.html)

In the introduction we looked at [some async Rust code][part_one] without
explaining anything about how it worked. That left us with several mysteries:
What are `async` functions and the "futures" they return? What does
[`join_all`] do? And how is [`tokio::time::sleep`] different from
[`std::thread::sleep`]?[^tokio_main]

[part_one]: playground://async_playground/tokio_10.rs
[`join_all`]: https://docs.rs/futures/latest/futures/future/fn.join_all.html
[`tokio::time::sleep`]: https://docs.rs/tokio/latest/tokio/time/fn.sleep.html
[`std::thread::sleep`]: https://doc.rust-lang.org/std/thread/fn.sleep.html

[^tokio_main]: You might also be wondering what [`#[tokio::main]`][tokio_main]
    does. The short answer is it's a ["procedural macro"][proc_macro] that sets
    up the Tokio "runtime" and then calls our `async fn main` in that
    "environment". We're not going to get into proc macros in this series, but
    we'll start to get an idea of what an "environment" means here when we
    create a global list of `Waker`s towards the end of this post, and we'll
    keep building on that with "tasks" in Part Two and "file descriptors" in
    Part Three.

[tokio_main]: https://docs.rs/tokio-macros/latest/tokio_macros/attr.main.html
[proc_macro]: https://doc.rust-lang.org/reference/procedural-macros.html

To answer those questions, we're going to translate each piece into normal,
non-async Rust code and stare at it for a while. We'll find that we can
replicate `foo` and `join_all` without too much trouble, but writing our own
`sleep` will be more complicated. [Let's go.][so_it_begins]

[so_it_begins]: https://youtu.be/QYSYAHDKtvM?t=17

## Foo

As a reminder, here's `foo` as an `async fn`:

```rust
LINK: Playground ## playground://async_playground/tokio_10.rs
async fn foo(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}
```

And here's `foo` as a regular, non-async function. I'll put everything on the
page, and then we'll break it down piece-by-piece. This is an exact, drop-in
replacement, so `main` hasn't changed at all. You can click the Playground
button and run it:

```rust
LINK: Playground ## playground://async_playground/foo.rs
fn foo(n: u64) -> Foo {
    let started = false;
    let duration = Duration::from_secs(1);
    let sleep = Box::pin(tokio::time::sleep(duration));
    Foo { n, started, sleep }
}

struct Foo {
    n: u64,
    started: bool,
    sleep: Pin<Box<tokio::time::Sleep>>,
}

impl Future for Foo {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if !self.started {
            println!("start {}", self.n);
            self.started = true;
        }
        if self.sleep.as_mut().poll(context).is_pending() {
            return Poll::Pending;
        }
        println!("end {}", self.n);
        Poll::Ready(())
    }
}
```

Starting from the top, `fn foo` is a regular function that returns a `Foo`
struct.[^convention] It calls [`tokio::time::sleep`], but it doesn't `.await`
the [`Sleep`] future that `sleep` returns.[^compiler_error] Instead, it stores
that future in the struct. We'll talk about [`Box::pin`] and
[`Pin<Box<_>>`][`Pin`] in a moment.

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

[^compiler_error]: It would be a [compiler error] to use `.await` in a
    non-async function.

[compiler error]: playground://async_playground/compiler_errors/await.rs

[`Box::pin`]: https://doc.rust-lang.org/std/boxed/struct.Box.html#method.pin
[`Pin`]: https://doc.rust-lang.org/std/pin/struct.Pin.html

The most important thing about `Foo` is that it implements the [`Future`]
trait. Here's how `Future` is defined in the standard library:

[`Future`]: https://doc.rust-lang.org/std/future/trait.Future.html

```rust
pub trait Future {
    type Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>;
}
```

The trait is just a couple lines of code, but it gives us three new types to
think about: [`Pin`], [`Context`], and [`Poll`]. We're going to focus on
`Poll`, so let's say a bit about `Context` and `Pin`, and then we'll set them
aside until later.

[`Context`]: https://doc.rust-lang.org/std/task/struct.Context.html
[`Poll`]: https://doc.rust-lang.org/std/task/enum.Poll.html

Each call to `Future::poll` receives a `Context` from the caller. When one
`poll` function calls another, like when `Foo::poll` calls `Sleep::poll`, it
passes that `Context` along. That's all we need to know until we get to the
[Wake](#wake) section below.

`Pin` is a wrapper type that wraps pointers. For now, if you'll forgive me,
we're going to close our eyes and imagine that `Pin` does _nothing at
all_.[^truth] We'll imagine that `Pin<Box<_>>` is just `Box<_>`,[^boxing] that
`Pin<&mut _>` is just `&mut _`, and that `Pin<Box<_>>::as_mut` is just
[`Box::as_mut`].[^as_mut] `Pin` actually solves an important problem, but that
problem will make more sense after we've had some practice writing futures.
We'll come back to it in the [Pin](#bonus__pin) section below.

[^truth]: As far as lies go, this one is pretty close to the truth. `Pin`'s job
    is to _prevent_ certain things in safe code, namely, moving certain futures
    after they've been polled. It's like how a shared reference prevents
    mutation. The reference doesn't _do_ much, but it represents permission.

[^boxing]: I'll be using `Box::pin` a bit more than usual in this series, as a
    shortcut to avoid talking about ["pin projection"][projection]. But note
    that most futures in Rust are _not_ heap allocated, at least not
    individually. This is different from coroutines in C++20, which are [heap
    allocated by default][cpp_coroutines].

[projection]: https://doc.rust-lang.org/std/pin/index.html#projections-and-structural-pinning
[cpp_coroutines]: https://pigweed.dev/docs/blog/05-coroutines.html
[`Box::as_mut`]: https://doc.rust-lang.org/std/boxed/struct.Box.html#method.as_mut

[^as_mut]: It's rare that we need to call `Box::as_mut` explicitly, because
    `&mut Box<T>` automatically converts to `&mut T` as needed. This is called
    ["deref coercion"][deref_coercion]. Similarly, Rust does automatic
    ["reborrowing"][reborrowing] of `&mut T` whenever we pass a long-lived
    mutable reference to a function that only needs a short-lived one, so that
    the long-lived reference isn't consumed unnecessarily. However, neither of
    those convenience features work through `Pin` today, and we often need to
    call `as_mut` explicitly when we're implementing `poll` "by hand". If
    `&pin` or maybe `&pinned` references [become a first-class language feature
    someday][pinned_places], that will make these examples shorter and less
    finicky.

[deref_coercion]: https://doc.rust-lang.org/book/ch15-02-deref.html#implicit-deref-coercions-with-functions-and-methods
[reborrowing]: https://github.com/rust-lang/reference/issues/788
[pinned_places]: https://without.boats/blog/pinned-places/

Ok, let's focus on `Poll`. It's an enum, and it looks like this:[^option]

[^option]: We said that futures have a lot in common with iterators, and this
    is another example. `Future::poll` returns `Poll`, [`Iterator::next`]
    returns [`Option`], and `Poll` and `Option` are very similar. `Ready` and
    `Some` are the variants that carry values, and `Pending` and `None` are the
    variants that don't. But note that their "order of appearance" is mirrored.
    An iterator returns `Some` any number of times until it eventually returns
    `None`, while a future returns `Pending` and number of times until it
    eventually returns `Ready`.

[`Iterator::next`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#tymethod.next
[`Option`]: https://doc.rust-lang.org/std/option/enum.Option.html

```rust
pub enum Poll<T> {
    Ready(T),
    Pending,
}
```

The most important thing that the `poll` function does is that it returns
either `Poll::Ready` or `Poll::Pending`. `Ready` means that the future is
finished with its work, and it includes the `Output` value, if any.[^no_output]
In this case, `poll` won't be called again.[^logic_error] `Pending` means that
the future isn't finished yet, and `poll` will be called again.

[^no_output]: `async fn foo` has no return value, so the `Foo` future has no
    `Output`. Rust represents no value with `()`, the empty tuple, also known
    as the "unit" type.

[^logic_error]: Yet another similarity between `Future` and `Iterator` is that
    calling `poll` again after it's returned `Ready` is like calling `next`
    again after it's returned `None`. Both of those are considered "logic
    errors", i.e. bugs in the caller. These functions are safe, so they're not
    allowed to corrupt memory or lead to other [undefined behavior], but they
    are allowed to panic, deadlock, or return "random" results.

[undefined behavior]: https://doc.rust-lang.org/nomicon/what-unsafe-does.html

_When_ will `poll` be called again, you might ask? The short answer is that we
need to be prepared for anything. Our `poll` function might get called over and
over again in a "busy loop" as long as it keeps returning `Pending`, and we
need it to behave correctly if that happens.[^not_busy] We'll get to the long
answer in the [Wake](#wake) section below.

[^not_busy]: However, if we [put an extra `println` in the `poll`
    function][foo_printing], we can see that it's not actually getting called
    in a busy loop. Interesting!

[foo_printing]: playground://async_playground/foo_printing.rs

Now let's look `Foo`'s implementation of the `Future` trait and the `poll`
function. Here it is again:

```rust
LINK: Playground ## playground://async_playground/foo.rs
impl Future for Foo {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if !self.started {
            println!("start {}", self.n);
            self.started = true;
        }
        if self.sleep.as_mut().poll(context).is_pending() {
            return Poll::Pending;
        }
        println!("end {}", self.n);
        Poll::Ready(())
    }
}
```

We saw that `poll`'s first job was returning `Ready` or `Pending`, but now we
can see that `poll` has a second job,[^three] the actual work of the future.
`Foo` is supposed to print some messages, and `poll` is where that printing
happens.

[^three]: In fact `poll` has three jobs. We'll get to the third in the
    [Sleep](#sleep) section.

There's an important compromise here: `poll` should do all the work that it can
get done quickly, but it shouldn't make the caller wait for an answer. It
should either return `Ready` immediately or return `Pending`
immediately.[^immediately] This compromise is the key to "driving" many futures
at the same time. It's what lets them make progress without waiting on each
other's work.

[^immediately]: When we're doing IO, "immediately" means that we shouldn't
    block. But when we're doing CPU-heavy work, like compression or
    cryptography, it's less clear what it means. The usual rule of thumb is
    that if a function does more than "a few milliseconds" of CPU work, it
    should either insert `.await` points to break up the work, or offload it
    with [`spawn_blocking`] or similar.

[`spawn_blocking`]: https://dtantsur.github.io/rust-openstack/tokio/task/fn.spawn_blocking.html

That means this implementation of `poll` is only correct if `Sleep::poll`
returns quickly. If we [add some timing and printing][timing], we can see that
it does. Now we can understand why [the `thread::sleep` mistake][intro_sleep]
in the introduction was 10x slower than the correct version. `thread::sleep`
doesn't return quickly. If we [use it in our `poll` function][same_result], we
get exactly the same result.

[timing]: playground://async_playground/foo_timing.rs?mode=release
[intro_sleep]: playground://async_playground/tokio_blocking.rs
[same_result]: playground://async_playground/foo_blocking.rs

`Foo` uses the `started` flag to make sure it only prints the start message
once, no matter how many times `poll` is called. It returns `Ready` when it
prints the end message, so it doesn't need to worry about `poll` getting called
again after that. The `started` flag makes `Foo` a "state machine" with two
states. In general, an `async` function needs a starting state and a state for
each of its `.await` points, so that the `poll` function knows where to "resume
execution". If we had more than two states, we could use an `enum` instead of a
`bool`. When we write an `async fn`, the compiler figures all of this out for
us, and that convenience is the main reason that async/await exists as a
language feature.[^unsafe_stuff]

[^unsafe_stuff]: Apart from the convenience, async/await also makes it possible
    to certain things in safe code that are `unsafe` when we do them in `poll`.
    We'll see that in the [Pin](#bonus__pin) section.

The 28 lines of code in this section are the most important lines in this
series, so it's worth taking the time to type them out by hand. Now that we
understand how to implement `foo`, let's implement `join_all`.

## Join

It might seem like [`join_all`] is doing something much more magical than
`foo`, but now that we've seen the moving parts of a future, it turns out we
already have everything we need. Here's `join_all` as a regular, non-async
function:[^always_was]

[^always_was]: In fact it's [defined this way upstream][upstream].

[upstream]: https://docs.rs/futures-util/0.3.30/src/futures_util/future/join_all.rs.html#102-105

```rust
LINK: Playground ## playground://async_playground/join.rs
fn join_all<F: Future>(futures: Vec<F>) -> JoinAll<F> {
    JoinAll {
        futures: futures.into_iter().map(Box::pin).collect(),
    }
}

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
`Vec<F::Output>`, which requires some more bookkeeping. This is left as an
exercise for the reader, as they say.

[^payload]: Specifically, when we call `.is_pending()` on the result of `poll`,
    we ignore any value that `Poll::Ready` might be carrying.

Onward!

## Sleep

We're on a roll here! It feels like we already have everything we need to
implement our own `sleep`:[^narrator]

[^narrator]: Narrator: They did not have everything they needed.

```rust
LINK: Playground ## playground://async_playground/sleep_forever.rs
fn sleep(duration: Duration) -> Sleep {
    let wake_time = Instant::now() + duration;
    Sleep { wake_time }
}

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
`context.waker()`, we get something called a [`Waker`].[^only_method] Then
calling either `waker.wake()` or `waker.wake_by_ref()` is how we ask to be
polled again. Those two methods do the same thing, and we'll use whichever one
is more convenient.[^efficiency]

[`Context`]: https://doc.rust-lang.org/std/task/struct.Context.html
[`Waker`]: https://doc.rust-lang.org/std/task/struct.Waker.html

[^only_method]: Handing out a `Waker` is currently all that `Context` does. An
    early version of the `Future` trait even had `poll` take a `Waker` directly
    instead of wrapping it in a `Context`, but the designers [wanted to leave
    open the possibility][possibility] of expanding the `poll` API in a
    backwards-compatible way. Note that `Waker` is `Clone` and `Send`, but
    `Context` is not.

[possibility]: https://github.com/rust-lang/rust/pull/59119

[^efficiency]: As we'll see in Part Two, the implementation of `Waker` is
    ultimately something that we can control. [In some
    implementations][waker_implementations], `Waker` is an `Arc` internally,
    and invoking a `Waker` might move that `Arc` into a global queue. In that
    case, `wake_by_ref` would need to clone the `Arc`, so `wake` saves an
    atomic operation on the refcount. This is a very small optimization, and we
    won't worry about it.

[waker_implementations]: https://without.boats/blog/wakers-i/

The simplest thing we can try is immediately asking to be polled again every
time we return `Pending`:

```rust
LINK: Playground ## playground://async_playground/sleep_busy.rs
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
LINK: Playground ## playground://async_playground/loop.rs
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
_always_ arrange to call `wake` somehow, even when they know the main loop is
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
LINK: Playground ## playground://async_playground/wakers.rs
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
LINK: Playground ## playground://async_playground/wakers.rs
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
LINK: Playground ## playground://async_playground/wakers.rs
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

## Bonus: Pin

Now that we know how to transform an `async fn` into a `Future` struct, we
can say a bit more about `Pin` and the problem that it solves. Imagine our
`async fn foo` took a reference internally for some reason:

```rust
LINK: Playground ## playground://async_playground/tokio_ref.rs
HIGHLIGHT: 2,3,5
async fn foo(n: u64) {
    let n_ref = &n;
    println!("start {n_ref}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n_ref}");
}
```

That compiles and runs just fine, and it looks like perfectly ordinary Rust
code. But what would the same change look like on our `Foo` future?

```rust
LINK: Playground ## playground://async_playground/compiler_errors/foo_ref.rs
HIGHLIGHT: 2,3
struct Foo {
    n: u64,
    n_ref: &u64,
    started: bool,
    sleep: Pin<Box<tokio::time::Sleep>>,
}
```

That doesn't compile:

```
LINK: Playground ## playground://async_playground/compiler_errors/foo_ref.rs
error[E0106]: missing lifetime specifier
 --> src/main.rs:3:12
  |
3 |     n_ref: &u64,
  |            ^ expected named lifetime parameter
```

What's the lifetime of `n_ref` supposed to be? The short answer is, there's no
good answer.[^longer] Self-referential borrows are generally illegal in Rust
structs, and there's no syntax for what `n_ref` is trying to do. If there were,
we'd have to answer some tricky questions about when we're allowed to mutate
`n` and when we're allowed to move `Foo`.[^quote]

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

But then, what sort of `Future` did the compiler generate for `async fn foo`
above? Why did that work? It turns out that Rust does [some very unsafe
things][erase] internally to erase inexpressible lifetimes like the one on
`n_ref`.[^unsafe_pinned] The job of the `Pin` pointer-wrapper-type is then to
encapsulate that unsafety, so that we can write custom futures like `JoinAll`
in safe code. The `Pin` struct works with the [`Unpin`] auto
trait,[^auto_traits] which is implemented for most concrete types but not for
the compiler-generated futures returned by `async` functions. Operations that
might let us move pinned objects are either gated by `Unpin`
([`DerefMut`][pin_deref_mut]) or marked `unsafe` ([`get_unchecked_mut`]).

[erase]: https://tmandry.gitlab.io/blog/posts/optimizing-await-1/#generators-as-data-structures

[^unsafe_pinned]: In fact, what the compiler is doing is so wildly unsafe
    that some of the machinery to make it formally sound [hasn't been
    implemented yet][unsafe_pinned].

[unsafe_pinned]: https://rust-lang.github.io/rfcs/3467-unsafe-pinned.html

[`Unpin`]: https://doc.rust-lang.org/std/marker/trait.Unpin.html

[^auto_traits]: ["Auto traits"] are implemented automatically by the
    compiler for types that qualify. The most familiar auto traits are the
    thread safety markers, `Send` and `Sync`. However, note that those two
    are `unsafe` traits, because implementing them inappropriately can lead
    to data races and other UB. In contrast, `Unpin` is _safe_, and types
    that don't implement it automatically (usually because they have
    generic type parameters that aren't required to be `Unpin`) can still
    safely opt into it. That's sound for two reasons: First, the main
    reason a type shouldn't implement `Unpin` is if it contains
    self-references and can't be moved, but we can't create types like that
    in safe code anyway. Second, even though `Unpin` lets us go from
    `Pin<&mut T>` to `&mut T`, we can't construct a pinned pointer to one
    of `T`'s fields ("pin projection") without either requiring that field
    to be `Unpin` ([`Pin::new`]) or writing `unsafe` code
    ([`Pin::new_unchecked`]).

["Auto traits"]: https://doc.rust-lang.org/reference/special-types-and-traits.html#auto-traits
[`Pin::new_unchecked`]: https://doc.rust-lang.org/std/pin/struct.Pin.html#method.new_unchecked
[`Pin::new`]: https://doc.rust-lang.org/std/pin/struct.Pin.html#method.new

[`get_unchecked_mut`]: https://doc.rust-lang.org/core/pin/struct.Pin.html#method.get_unchecked_mut
[pin_deref_mut]: https://doc.rust-lang.org/core/pin/struct.Pin.html#impl-DerefMut-for-Pin%3CPtr%3E

This is all we're going to say about `Pin`, because we're going to move on to
tasks (Part Two) and IO (Part Three), and the nitty gritty details of pinning
aren't going to come up. But if you want the whole story, start with [this post
by the inventor of `Pin`][pin_post] and then read through [the official `Pin`
docs][pin_docs].

[pin_post]: https://without.boats/blog/pin
[pin_docs]: https://doc.rust-lang.org/std/pin

## Bonus: Cancellation

Async functions look and feel a lot like regular functions, but they have a
certain extra superpower, and there's another superpower that they're missing.

The extra superpower they have is cancellation. When we call an ordinary
function in non-async code, there's no general way for us to put a timeout on
that call.[^thread_cancellation] But we can cancel any future by not polling it
again. Tokio provides [`tokio::time::timeout`] for this, and we already have
the tools to implement our own version:

[^thread_cancellation]: We can spawn a new thread and call the function on that
    thread, using a channel for example to wait for its return value with a
    timeout. If the timeout passes, our main thread can go do something else,
    but there's no general way to interrupt the background thread. One reason
    this feature doesn't exist is that, if the interrupted thread was holding
    any locks, those locks would never get unlocked. Lots of common libc
    functions like `malloc` use locks internally, so forcefully killing threads
    would tend to break the whole world. This is also why [`fork` is difficult
    to use correctly][fork].

[fork]: https://www.microsoft.com/en-us/research/uploads/prod/2019/04/fork-hotos19.pdf
[`tokio::time::timeout`]: https://docs.rs/tokio/latest/tokio/time/fn.timeout.html

```rust
LINK: Playground ## playground://async_playground/timeout.rs
struct Timeout<F> {
    sleep: Pin<Box<tokio::time::Sleep>>,
    inner: Pin<Box<F>>,
}

impl<F: Future> Future for Timeout<F> {
    type Output = Option<F::Output>;

    fn poll(
        mut self: Pin<&mut Self>,
        context: &mut Context,
    ) -> Poll<Self::Output> {
        // Check whether the inner future is finished.
        if let Poll::Ready(output) = self.inner.as_mut().poll(context) {
            return Poll::Ready(Some(output));
        }
        // Check whether time is up.
        if self.sleep.as_mut().poll(context).is_ready() {
            return Poll::Ready(None);
        }
        // Still waiting.
        Poll::Pending
    }
}

fn timeout<F: Future>(duration: Duration, inner: F) -> Timeout<F> {
    Timeout {
        sleep: Box::pin(tokio::time::sleep(duration)),
        inner: Box::pin(inner),
    }
}
```

## Bonus: Recursion

The missing superpower is recursion. If an async function tries to call itself:

```rust
LINK: Playground ## playground://async_playground/compiler_errors/recursion.rs
async fn factorial(n: u64) -> u64 {
    if n == 0 {
        1
    } else {
        n * factorial(n - 1).await
    }
}
```

That's a compiler error:

```
LINK: Playground ## playground://async_playground/compiler_errors/recursion.rs
error[E0733]: recursion in an async fn requires boxing
 --> recursion.rs:1:1
  |
1 | async fn factorial(n: u64) -> u64 {
  | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
...
5 |         n * factorial(n - 1).await
  |             ---------------------- recursive call here
  |
  = note: a recursive `async fn` must introduce indirection such as `Box::pin` to avoid an infinitely sized future
```

When regular functions call other functions, they allocate space dynamically on
the "call stack". But when futures await other futures, they get compiled into
structs that contain other structs, and struct sizes are static.[^stackless] If
an an async function calls itself, it has to `Box` the recursive future before
awaiting it:

[^stackless]: In other words, Rust futures are "stackless coroutines". For
    comparison, "goroutines" in Go are "stackful", and they can do recursion
    without any extra steps.

```rust
LINK: Playground ## playground://async_playground/boxed_recursion.rs
async fn factorial(n: u64) -> u64 {
    if n == 0 {
        1
    } else {
        let recurse = Box::pin(factorial(n - 1));
        n * recurse.await
    }
}
```

This works, but it requires heap allocation.

---

<div class="prev-next-arrows">
    <div><a href="async_intro.html">← Introduction</a></div>
    <div class="space"> </div><div>
    <a href="async_tasks.html"> Part Two: Tasks →</a>
</div>
