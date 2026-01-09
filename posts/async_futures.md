# Async Rust, Part One: Futures
###### 2024 October 23<sup>rd</sup>

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

In the introduction we looked at [an example of async Rust][part_one] without
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
    we'll start to get an idea of what goes into an async "environment" when we
    create a global set of `Waker`s in the [Main](#main) section below, and
    we'll keep building on that with "tasks" in [Part Two] and "file
    descriptors" in [Part Three].

[Part Two]: async_tasks.html
[Part Three]: async_io.html

[tokio_main]: https://docs.rs/tokio-macros/latest/tokio_macros/attr.main.html
[proc_macro]: https://doc.rust-lang.org/reference/procedural-macros.html

To answer those questions, we're going to translate each of those pieces into
normal, non-async Rust code. We'll find that we can replicate `foo` and
`join_all` without too much trouble, but writing our own `sleep` will be more
complicated. [Let's go.][so_it_begins]

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
page, and then we'll break it down piece-by-piece. This is a drop-in
replacement, and `main` hasn't changed at all. You can click the Playground
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
that future in the `Foo` struct. We'll talk about [`Box::pin`] and
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

Implementing the [`Future`] trait is what makes `Foo` a future. Here's the
entire `Future` trait from the standard library:

[`Future`]: https://doc.rust-lang.org/std/future/trait.Future.html

```rust
pub trait Future {
    type Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>;
}
```

The trait itself is only a couple lines of code, but it comes with three new
types: [`Pin`], [`Context`], and [`Poll`]. We're going to focus on `Poll`, so
we'll say a bit about `Pin` and `Context` and then set them aside until later.

[`Context`]: https://doc.rust-lang.org/std/task/struct.Context.html
[`Poll`]: https://doc.rust-lang.org/std/task/enum.Poll.html

`Pin<&mut T>` is the pointer type we use to call `Future::poll`. The owned
version of that is `Pin<Box<T>>`, which we construct with [`Box::pin`][^boxing]
and borrow with [`.as_mut()`][pin_as_mut].[^as_mut] That's all we need to know
until we get to the [Pin][pin_section] section below. `Pin` actually solves a
crucial problem for async Rust,[^permissions] but that problem will make more
sense once we have some practice writing futures.

[^boxing]: There are [other ways][pin_macro] to pin things, but we'll stick
    with `Box::pin` throughout this series, as a shortcut to avoid talking
    about ["pin projection"][projection]. The original `async fn foo` actually
    doesn't `Box` the `Sleep` future, and most futures in Rust aren't heap
    allocated, at least not individually. This is different from coroutines in
    C++20, which are [heap allocated by default][cpp_coroutines].

[pin_macro]: https://doc.rust-lang.org/stable/std/pin/macro.pin.html
[projection]: https://doc.rust-lang.org/std/pin/index.html#projections-and-structural-pinning
[cpp_coroutines]: https://pigweed.dev/docs/blog/05-coroutines.html

[pin_as_mut]: https://doc.rust-lang.org/std/pin/struct.Pin.html#method.as_mut
[pin_section]: #bonus__pin

[^as_mut]: We don't often call `Box::as_mut` explicitly in non-async Rust,
    because `&mut Box<T>` automatically converts to `&mut T` as needed. This is
    called ["deref coercion"][deref_coercion]. Similarly, Rust automatically
    ["reborrows"][reborrowing] `&mut T` whenever we pass a long-lived mutable
    reference to a function that only needs a short-lived one, so that the
    long-lived reference isn't consumed unnecessarily. (`&mut` references [are
    not `Copy`][not_copy].) However, neither of those convenience features
    works through `Pin` today, and we often do need to call `Pin::as_mut`
    explicitly when we're implementing `Future` "by hand". If [`&pin` or maybe
    `&pinned` references][pinned_places] became a first-class language feature
    someday, that would make these examples shorter and less finicky.

[deref_coercion]: https://doc.rust-lang.org/book/ch15-02-deref.html#implicit-deref-coercions-with-functions-and-methods
[reborrowing]: https://github.com/rust-lang/reference/issues/788
[not_copy]: https://doc.rust-lang.org/std/marker/trait.Copy.html#impl-Copy-for-%26T
[pinned_places]: https://without.boats/blog/pinned-places/

[^permissions]: `Pin` makes sure that safe code can't move certain futures
    after they've been polled. We'll get into why that's important
    [below][pin_section]. As for _how_ `Pin` does that, it's like how a shared
    reference prevents mutation. It doesn't really _do_ anything (at runtime),
    but it represents permissions in the type system (at compile time).

Each call to `Future::poll` receives a `Context` from the caller. When one
`poll` function calls another, like when `Foo::poll` calls `Sleep::poll`, it
passes that `Context` along. That's all we need to know until we get to the
[Wake](#wake) section below.

Ok, let's focus on `Poll`. It's an enum, and it looks like this:[^option]

[^option]: We said that futures have a lot in common with iterators, and this
    is another example. `Future::poll` returns `Poll`, [`Iterator::next`]
    returns [`Option`], and `Poll` and `Option` are very similar. `Ready` and
    `Some` are the variants that carry values, and `Pending` and `None` are the
    variants that don't. But note that their "order of appearance" is mirrored.
    An iterator returns `Some` any number of times until it eventually returns
    `None`, while a future returns `Pending` any number of times until it
    eventually returns `Ready`.

[`Iterator::next`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#tymethod.next
[`Option`]: https://doc.rust-lang.org/std/option/enum.Option.html

```rust
pub enum Poll<T> {
    Ready(T),
    Pending,
}
```

The first job of the `poll` function is to return either `Poll::Ready` or
`Poll::Pending`. Returning `Ready` means that the future has finished its work,
and it includes the `Output` value, if any.[^no_output] In that case, `poll`
won't be called again.[^logic_error] Returning `Pending` means that the future
isn't finished yet, and `poll` will be called again.

[^no_output]: `async fn foo` has no return value, so the `Foo` future has no
    `Output`. Rust represents no value with `()`, the empty tuple, also known
    as the "unit" type.

[^logic_error]: Yet another similarity between `Future` and `Iterator` is that
    calling `Future::poll` again after it's returned `Ready` is a lot like
    calling `Iterator::next` again after it's returned `None`. Both of these
    are considered "logic errors", i.e. bugs in the caller. These functions are
    safe, so they're not allowed to corrupt memory or lead to other [undefined
    behavior], but they are allowed to panic, deadlock, or return "random"
    results.

[undefined behavior]: https://doc.rust-lang.org/nomicon/what-unsafe-does.html

_When_ will `poll` be called again, you might ask? The short answer is that we
need to be prepared for anything. Our `poll` function might get called over and
over again in a "busy loop" as long as it keeps returning `Pending`, and we
need it to behave correctly if that happens.[^not_busy] We'll get to the long
answer in the [Wake](#wake) section below.

[^not_busy]: If we [add an extra `println`][foo_printing], we can see that
    `poll` isn't actually getting called in a busy loop here. But there'll be a
    few times in this series where it does, including the [Wake](#wake) section
    below.

[foo_printing]: playground://async_playground/foo_printing.rs

Now let's look at `Foo`'s implementation of the `Future` trait and the `poll`
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

We saw that `poll`'s first job was to return `Ready` or `Pending`, and now we
can see that `poll` has a second job, the actual work of the future.[^three]
`Foo` wants to print some messages and sleep, and `poll` is where the printing
and sleeping happen.

[^three]: In fact `poll` has three jobs. We'll get to the third in the
    [Sleep](#sleep) section.

There's an important compromise here: `poll` should do all the work that it can
get done quickly, but it shouldn't make the caller wait for an answer. It
should either return `Ready` immediately or return `Pending`
immediately.[^immediately] This compromise is the key to "driving" more than
one future at the same time. It's what lets them make progress without blocking
each other.

[^immediately]: "Immediately" means that we shouldn't do any blocking sleeps or
    blocking IO in a `poll` function or an `async fn`. But when we're doing
    CPU-heavy work, like compression or cryptography, it's less clear what it
    means. The usual rule of thumb is that, if a function does more than "a few
    milliseconds" of CPU work, it should either offload that work using
    something like [`tokio::task::spawn_blocking`], or insert `.await` points
    using something like [`tokio::task::yield_now`].

[`tokio::task::spawn_blocking`]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
[`tokio::task::yield_now`]: https://docs.rs/tokio/latest/tokio/task/fn.yield_now.html

To follow this rule, `Foo::poll` has to trust that `Sleep::poll` will return
quickly. If we [add some timing and printing][timing], we can see that it does.
The ["important mistake"][intro_sleep] we made in the introduction with
`thread::sleep` broke this rule, and our futures ran back-to-back instead of at
the same time.[^concurrently] If we [make the same mistake in
`Foo::poll`][same_result], we get the same result. Doing a blocking sleep in
`poll` makes the caller wait for an answer, and it can't poll any other futures
while it's waiting.

[timing]: playground://async_playground/foo_timing.rs?mode=release
[intro_sleep]: playground://async_playground/tokio_blocking.rs
[same_result]: playground://async_playground/foo_blocking.rs

[^concurrently]: In other words, "serially" instead of "concurrently".

`Foo` uses the `started` flag to make sure it only prints the start message
once, no matter how many times `poll` is called. It doesn't need an `ended`
flag, though, because `poll` won't be called again after it returns
`Ready`.[^two_reasons] The `started` flag makes `Foo` a "state machine" with
two states. In general an `async` function needs one starting state and another
state for each of its `.await` points, so that its `poll` function can know
where to "resume execution". If we had more than two states, we could use an
`enum` instead of a `bool`. When we write an `async fn`, the compiler takes
care of all of this for us, and that convenience is the main reason that
async/await exists as a language feature.[^unsafe_stuff]

[^two_reasons]: That's what keeps us from printing the end message more than
    once, and also from breaking the very same rule by polling the `Sleep`
    future again after it's returned `Ready`. `Sleep` happens to be lenient
    about this (it'll just return `Ready` again), but compiler-generated `async
    fn` futures panic if we break this rule, and [some futures][fuse] never
    return `Ready` again.

[fuse]: https://docs.rs/futures/latest/futures/future/trait.FutureExt.html#method.fuse

[^unsafe_stuff]: Async/await also makes it possible to do certain things in
    safe code that would be `unsafe` in `poll`. We'll see that in the
    [Pin](#bonus__pin) section.

## Join

Now that we know how to implement a basic future, let's look at
[`join_all`].[^either_version] It might seem like `join_all` is doing something
much more magical than `foo`, but it turns out we already have everything we
need to make it work. Here's `join_all` as a regular, non-async
function:[^always_was]

[^either_version]: We'll put `async fn foo` back in to keep the examples short,
    but either version of `foo` would work going forward.

[^always_was]: In fact it's [implemented as a regular function][upstream]
    upstream in the [`futures`] crate.

[upstream]: https://docs.rs/futures-util/0.3.30/src/futures_util/future/join_all.rs.html#102-105
[`futures`]: https://docs.rs/futures/latest/futures

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

Again our non-async `join_all` function returns a struct that implements
`Future`, and the real work happens in `Future::poll`. There's `Box::pin`
again, but we'll keep ignoring it.

In the `poll` function, [`Vec::retain_mut`] does all the heavy lifting. It's a
standard `Vec` helper method that takes a closure argument, calls that closure
on each element, and drops the elements that return `false`.[^algorithm] This
removes each child future once it returns `Ready`, following the rule that we
shouldn't poll them again after that.

[`Vec::retain_mut`]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.retain_mut

[^algorithm]: If we did this by calling `remove` in a loop, it would take
    O(n<sup>2</sup>) time, because `remove` is O(n). But `retain_mut` uses a
    clever algorithm that "walks" two pointers through the `Vec` and moves each
    element at most once.

[`Vec::remove`]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.remove

There's nothing else new here. From the outside, running all these futures at
the same time seemed like magic, but on the inside, all we're doing is calling
`poll` on the elements of a `Vec`. This is the flip side of the compromise we
talked about above. If we can trust that each call to `poll` returns quickly,
then one loop can drive lots of futures.

Note that I'm taking a shortcut here by ignoring the outputs of child futures.
I'm getting away with that because we're only using `join_all` with `foo`,
which has no output. The real `join_all` returns `Vec<F::Output>`, which
requires some more bookkeeping. This is left as an exercise for the reader, as
they say.[^not_either_version]

[^not_either_version]: As we did `foo`, we're going to put the original
    `join_all` back in going forward. This time though, the two versions aren't
    exactly the same, even apart from the shortcut we just mentioned. We'll
    make another "important mistake" in the [Main](#main) section below, which
    only works with the version of `join_all` from the [`futures`] crate.

## Sleep

We're on a roll here! It feels like we already have everything we need to
implement our own `sleep`:[^narrator]

[^narrator]: We do not.

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

(Playground running…)

Hmm. This compiles cleanly, and the logic in `poll` looks right, but running it
prints the "start" messages and then hangs forever. If we [add more
prints][sleep_forever_dbg], we can see that each `Sleep` gets polled once at
the start and then never again. What are we missing?

[sleep_forever_dbg]: playground://async_playground/sleep_forever_dbg.rs

It turns out that `poll` has [three jobs][poll_docs], and so far we've only
seen two. First, `poll` does as much work as it can without blocking. Then,
`poll` returns `Ready` if it's finished or `Pending` if it's not. But finally,
whenever `poll` is about to return `Pending`, it needs to "schedule a wakeup".
Ah, that's what we're missing.

[poll_docs]: https://doc.rust-lang.org/std/future/trait.Future.html#tymethod.poll

The reason we didn't run into this before is that `Foo` and `JoinAll` only
return `Pending` when another future has returned `Pending` to them, which
means a wakeup is already scheduled. But `Sleep` is what we call a "leaf"
future. There are no other futures below it,[^upside_down] and it needs to wake
itself.

[^upside_down]: Trees in computing are upside down for some reason.

## Wake

It's time to look more closely at [`Context`]. If we call `context.waker()`, it
returns a [`Waker`].[^only_method] Calling either [`waker.wake()`][wake] or
[`waker.wake_by_ref()`][wake_by_ref] is how we ask to get polled again. Those
two methods do the same thing, and we'll use whichever one is more
convenient.[^efficiency]

[`Context`]: https://doc.rust-lang.org/std/task/struct.Context.html
[`Waker`]: https://doc.rust-lang.org/std/task/struct.Waker.html
[wake]: https://doc.rust-lang.org/std/task/struct.Waker.html#method.wake
[wake_by_ref]: https://doc.rust-lang.org/std/task/struct.Waker.html#method.wake_by_ref

[^only_method]: Handing out a `Waker` is currently all that `Context` does. An
    early version of the `Future` trait had `poll` take a `Waker` directly
    instead of wrapping it in a `Context`, but the designers [wanted to leave
    open the possibility][possibility] of expanding the `poll` API in a
    backwards-compatible way. Note that `Waker` is `Clone` and `Send`, but
    `Context` is not.

[possibility]: https://github.com/rust-lang/rust/pull/59119

[^efficiency]: As we'll see in Part Two, the implementation of `Waker` is
    ultimately something that we can control. [In some
    implementations][waker_implementations], `Waker` is an `Arc` internally,
    and invoking a `Waker` might move that `Arc` into a global queue. In that
    case, `wake_by_ref` would need to clone the `Arc`, and `wake` would save an
    atomic operation on the refcount. This is a micro-optimization, and we
    won't worry about it.

[waker_implementations]: https://without.boats/blog/wakers-i/

The simplest thing we can try is asking to get polled again immediately every
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
forever" problem is solved, but we've replaced it with a "busy loop" problem.
This program calls `poll` over and over as fast as it can, burning 100% CPU
until it exits. We can see this indirectly by [counting the number of times
`poll` gets called][sleep_busy_dbg],[^poll_count] or we can measure it directly
[using tools like `perf` on Linux][perf].

[sleep_busy_dbg]: playground://async_playground/sleep_busy_dbg.rs?mode=release

[^poll_count]: When I run this on the Playground, I see about 20&nbsp;_million_
    calls in total.

[perf]: https://github.com/oconnor663/jacko.io/blob/master/posts/async_playground/perf_stat.sh

What we really want is to invoke the `Waker` later, when it's actually time to
wake up, but we can't use `thread::sleep` in `poll`. One thing we could do is
spawn another thread to `thread::sleep` for us and then call `wake`.[^send] If
we [did that in every call to `poll`][same_crash], we'd run into the same
too-many-threads crash from the introduction. However, we could work around
that by [spawning a shared thread and using a channel to send `Waker`s to
it][shared_thread]. That's actually a viable implementation, but there's
something unsatisfying about it. The main thread of our program is already
spending most of its time sleeping. Why do we need two sleeping threads? Why
isn't there a way to tell our main thread to wake up at a specific time?

[^send]: Note that `Waker` is `Send`, so this is allowed.

Well to be fair, there is a way, that's what `tokio::time::sleep` is. But if we
really want to write our own `sleep`, and we don't want to spawn an extra
thread to make it work, then it turns out we also need to get rid of
`#[tokio::main]` and write our own `main` function.

[same_crash]: playground://async_playground/sleep_many_threads.rs
[shared_thread]: playground://async_playground/sleep_one_thread.rs

## Main

To call `poll` from `main`, we'll need a `Context` to pass in. We can make one
with [`Context::from_waker`], which means we need a `Waker`. There are a few
different ways to make one,[^make_a_waker] but for now we just need a
placeholder, so we'll use a helper function called [`Waker::noop`].[^noop] Once
we've built a `Context`, we can call `poll` in a loop:

[`Context::from_waker`]: https://doc.rust-lang.org/std/task/struct.Context.html#method.from_waker

[^make_a_waker]: We'll get to these in [the Waker section of Part
    Two](async_tasks.html#waker).

[^noop]: "Noop" is short for "no operation", i.e. "do nothing". `Waker::noop`
    was stabilized in Rust 1.85, and the original version of this post used
    [`futures::task::noop_waker`] instead.

[`futures::task::noop_waker`]: https://docs.rs/futures/latest/futures/task/fn.noop_waker.html

[`Waker::noop`]: https://doc.rust-lang.org/std/task/struct.Waker.html#method.noop

```rust
LINK: Playground ## playground://async_playground/loop.rs
fn main() {
    let mut futures = Vec::new();
    for n in 1..=10 {
        futures.push(foo(n));
    }
    let mut joined_future = Box::pin(future::join_all(futures));
    let waker = Waker::noop();
    let mut context = Context::from_waker(&waker);
    while joined_future.as_mut().poll(&mut context).is_pending() {
        // Busy loop!
    }
}
```

This works! Though we still have the "busy loop" problem from above. But before
we fix that, we need to make another important mistake:

Since this version of our main loop[^event_loop] never stops polling, and since
our `Waker` does nothing, you might wonder whether invoking the `Waker` in
`Sleep::poll` actually matters. Surprisingly, it does matter. If we delete that
line, [things appear to work at first][loop_forever_10]. But when we bump the
number of jobs from ten to a hundred, [our futures never wake
up][loop_forever_100]. What we're seeing is that, even though _our_ `Waker`
does nothing, there are _other_ `Waker`s hidden in our program. When
[`futures::future::JoinAll`] has many child futures,[^cutoff] it creates its
own `Waker`s internally, which lets it avoid polling children that haven't
asked for a wakeup. That's more efficient than polling all of them every time,
but it also means that children who never invoke their own `Waker` will never
get polled again. This sort of thing is why a `Pending` future must _always_
arrange to invoke its `Waker`.[^pending]

[^event_loop]: This sort of thing is often called an "event loop", but right
    now all we have is sleeps, and those aren't really events. We'll build a
    proper event loop in [Part Three].

[loop_forever_10]: playground://async_playground/loop_forever_10.rs
[loop_forever_100]: playground://async_playground/loop_forever_100.rs
[`futures::future::JoinAll`]: https://docs.rs/futures/latest/futures/future/struct.JoinAll.html

[^cutoff]: As of `futures` v0.3.30, [the exact cutoff is
    31](https://github.com/rust-lang/futures-rs/blob/0.3.30/futures-util/src/future/join_all.rs#L35).

[^pending]: We don't usually worry about this, because most futures in ordinary
    application code aren't "leaf" futures. Most futures (like `Foo` and
    `JoinAll` and any `async fn`) only return `Pending` when other futures
    return `Pending` to them, so they can usually assume that those other
    futures have scheduled a wakeup. However, there are exceptions. The
    [`futures::pending!`][pending_macro] macro explicitly returns `Pending`
    _without_ scheduling a wakeup.

[pending_macro]: https://docs.rs/futures/latest/futures/macro.pending.html

Ok, back to `main`. Let's fix the busy loop problem. We want `main` to
`thread::sleep` until the next wake time, which means we need a way for
`Sleep::poll` to send `Waker`s and wake times to `main`. We'll use a global
variable for this, and we'll wrap it in a `Mutex`, so that safe code can modify
it:[^thread_local]

[^thread_local]: Real async runtimes like Tokio use
    [`thread_local!`][thread_local] instead of globals for this. That's more
    efficient, and it also lets them run more than one event loop in the same
    program. `Mutex` is more familiar, though, and it's good enough for this
    series.

[thread_local]: playground://async_playground/thread_local.rs

```rust
LINK: Playground ## playground://async_playground/wakers.rs
static WAKE_TIMES: Mutex<BTreeMap<Instant, Vec<Waker>>> =
    Mutex::new(BTreeMap::new());
```

This is a sorted map from wake times to `Waker`s.[^vec] `Sleep::poll` can
insert its `Waker` into this map using [`BTreeMap::entry`]:[^or_default]

[^vec]: Note that the value type here is `Vec<Waker>` instead of just `Waker`,
    because there might be more than one `Waker` for a given `Instant`. This is
    unlikely on Linux and macOS, where the resolution of `Instant::now()` is
    measured in nanoseconds, but the resolution on Windows is 15.6
    *milli*seconds.

[`BTreeMap::entry`]: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html#method.entry

[^or_default]: [`or_default`] creates an empty `Vec` if nothing was there
    before.

[`or_default`]: https://doc.rust-lang.org/std/collections/btree_map/enum.Entry.html#method.or_default

```rust
LINK: Playground ## playground://async_playground/wakers.rs
HIGHLIGHT: 5-7
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
```

After polling, our main loop can read the first key from this sorted map to get
the earliest wake time. Then it can `thread::sleep` until that time, fixing the
busy loop problem.[^hold_lock] Then it invokes all the `Waker`s whose wake time
has arrived, before looping and polling again:

[^hold_lock]: We're holding the `WAKE_TIMES` lock while we sleep here, which is
    slightly sketchy, but it doesn't matter in this single-threaded example. A
    real multithreaded runtime would use [`thread::park_timeout`] or similar
    instead of sleeping, so that other threads could trigger an early wakeup.

[`thread::park_timeout`]: https://doc.rust-lang.org/std/thread/fn.park_timeout.html

```rust
LINK: Playground ## playground://async_playground/wakers.rs
HIGHLIGHT: 10-22
fn main() {
    let mut futures = Vec::new();
    for n in 1..=10 {
        futures.push(foo(n));
    }
    let mut joined_future = Box::pin(future::join_all(futures));
    let waker = Waker::noop();
    let mut context = Context::from_waker(&waker);
    while joined_future.as_mut().poll(&mut context).is_pending() {
        // The joined future is Pending. Sleep until the next wake time.
        let mut wake_times = WAKE_TIMES.lock().unwrap();
        let next_wake = wake_times.keys().next().expect("sleep forever?");
        thread::sleep(next_wake.saturating_duration_since(Instant::now()));
        // We just woke up. Invoke all the Wakers whose time has come.
        while let Some(entry) = wake_times.first_entry()
            && *entry.key() <= Instant::now()
        {
            entry.remove().into_iter().for_each(Waker::wake);
        }
        // Loop and poll again.
    }
}
```

This works! We solved the busy loop problem, and we didn't need to spawn any
extra threads. This is what it takes to write our own `sleep`.

This is sort of the end of Part One. In [Part Two] we'll expand this main loop
to implement "tasks". However, now that we understand how futures work, there
are a few "bonus" topics that we finally have the tools to talk about, at least
briefly. The following sections aren't required for Parts Two and Three.

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
structs, and there's no syntax for what `n_ref` is trying to do. One big
problem is that, if `n_ref` was pointing to `n`, and then we moved the whole
`Foo`, the _new_ `n_ref` would still be pointing to the _old_ `n`.[^quote]

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
above? Why did that work? Rust does [some very unsafe things][erase] internally
to erase inexpressible lifetimes like the one on `n_ref`.[^unsafe_pinned] The
job of the `Pin` pointer-wrapper-type is then to encapsulate that unsafety, so
that we can write custom futures like `JoinAll` in safe code. The `Pin` struct
works with the [`Unpin`] auto trait,[^auto_traits] which is implemented for
most concrete types but not for the compiler-generated futures returned by
`async` functions. Operations that might let us move pinned objects are either
gated by `Unpin` (like [`DerefMut`][pin_deref_mut]) or marked `unsafe` (like
[`get_unchecked_mut`]). It turns out that our extensive use of `Box::pin` in
the examples above meant that all our futures were automatically `Unpin`, so
`DerefMut` worked for our `Pin<&mut Self>` references, and we could mutate
members like `self.started` and `self.futures` without thinking about it.

[erase]: https://tmandry.gitlab.io/blog/posts/optimizing-await-1/#generators-as-data-structures

[^unsafe_pinned]: In fact, what the compiler is doing is so wildly unsafe that
    some of the machinery that will make it formally sound [hasn't been
    implemented yet][unsafe_pinned].

[unsafe_pinned]: https://rust-lang.github.io/rfcs/3467-unsafe-pinned.html

[`Unpin`]: https://doc.rust-lang.org/std/marker/trait.Unpin.html

[^auto_traits]: ["Auto traits"] are implemented automatically by the compiler
    for types that qualify. The most familiar auto traits are the thread safety
    markers, `Send` and `Sync`. However, note that those two are `unsafe`
    traits, because implementing them inappropriately can lead to data races
    and other UB. In contrast, `Unpin` is _safe_, and types that don't
    implement it automatically (usually because they have generic type
    parameters that aren't required to be `Unpin`) can still safely opt into
    it. That's sound for two reasons: First, a type shouldn't implement `Unpin`
    if it contains self-references and can't be moved, but we can't make
    self-references in safe code anyway. Second, even though `Unpin` lets us go
    from `Pin<&mut T>` to `&mut T`, we still can't construct a pinned pointer
    to one of `T`'s fields (["pin projection"][projection]) without either
    requiring that field to be `Unpin` ([`Pin::new`]) or writing `unsafe` code
    ([`Pin::new_unchecked`]).

["Auto traits"]: https://doc.rust-lang.org/reference/special-types-and-traits.html#auto-traits
[`Pin::new_unchecked`]: https://doc.rust-lang.org/std/pin/struct.Pin.html#method.new_unchecked
[`Pin::new`]: https://doc.rust-lang.org/std/pin/struct.Pin.html#method.new

[`get_unchecked_mut`]: https://doc.rust-lang.org/core/pin/struct.Pin.html#method.get_unchecked_mut
[pin_deref_mut]: https://doc.rust-lang.org/core/pin/struct.Pin.html#impl-DerefMut-for-Pin%3CPtr%3E

That's all we're going to say about `Pin`, because the nitty gritty details
aren't necessary for tasks ([Part Two]) or IO ([Part Three]). But if you want
the whole story, start with [this post by the inventor of `Pin`][pin_post] and
then read [the official `Pin` docs][pin_docs].

[pin_post]: https://without.boats/blog/pin
[pin_docs]: https://doc.rust-lang.org/std/pin

## Bonus: Cancellation

Async functions look and feel a lot like regular functions, but they have a
certain extra superpower, and there's another superpower they're missing.

The extra superpower is cancellation. When we call an ordinary function in
non-async code, there's no general way for us to put a timeout on that
call.[^thread_cancellation] But we can cancel any future by just&hellip;not
polling it again. Tokio provides [`tokio::time::timeout`] for this, and we
already have the tools to implement our own version:

[`tokio::time::timeout`]: https://docs.rs/tokio/latest/tokio/time/fn.timeout.html

[^thread_cancellation]: We could spawn a new thread and call any function we
    like on that thread, for example using a channel to wait for its return
    value with a timeout. But if that timeout passes, there's no general way
    for us to stop the thread. One reason this feature doesn't exist (actually
    [it does exist on Windows][TerminateThread], but [you should never use
    it][old_new_thing]) is that, if the interrupted thread was holding any
    locks, those locks would never get unlocked. Lots of common libc functions
    like `malloc` use locks internally, so forcefully killing threads tends to
    corrupt the whole process. This is also why [`fork` is difficult to use
    correctly][fork].

[fork]: https://www.microsoft.com/en-us/research/uploads/prod/2019/04/fork-hotos19.pdf
[TerminateThread]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminatethread#remarks
[old_new_thing]: https://devblogs.microsoft.com/oldnewthing/20150814-00/?p=91811

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
        // Check whether time is up.
        if self.sleep.as_mut().poll(context).is_ready() {
            return Poll::Ready(None);
        }
        // Check whether the inner future is finished.
        if let Poll::Ready(output) = self.inner.as_mut().poll(context) {
            return Poll::Ready(Some(output));
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

This `timeout` wrapper works with _any_ async function. Regular functions have
no equivalent.[^footgun]

[^footgun]: Cancellation is a superpower, but it [can also be a
    footgun][too_easy]. Every `.await` is a potential cancellation point, so
    simple logic like "do A then B" can surprise you by quitting halfway
    through, if the caller happens to use a `timeout` or a
    [`select!`][select_macro]. That's sort of like an error or a panic showing
    up halfway through, but those are more likely to terminate the program or
    leave noisy logs, while cancellation is silent. Also, errors and panics
    come from the call*ee* ("the devil you know"), while cancellation comes
    from the call*er* ("the devil you don't").

[too_easy]: https://sunshowers.io/posts/cancelling-async-rust/
[select_macro]: https://docs.rs/futures/latest/futures/macro.select.html

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
…
5 |         n * factorial(n - 1).await
  |             ---------------------- recursive call here
  |
  = note: a recursive `async fn` must introduce indirection such as `Box::pin` to avoid an infinitely sized future
```

When regular functions call each other, they allocate space dynamically on the
"call stack". But when async functions `.await` each other, they get compiled
into structs that contain other structs, and struct sizes are
static.[^stackless] If an async function calls itself, it has to `Box` the
recurring future before it `.await`s it:

[^stackless]: In other words, Rust futures are "stackless coroutines". In Go on
    the other hand, "goroutines" are "stackful". They can do recursion without
    any extra steps.

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

Ok, enough of all that, on to "tasks".

---

<div class="prev-next-arrows">
    <div><a href="async_intro.html">← Introduction</a></div>
    <div class="space"> </div>
    <div><a href="async_tasks.html"> Part Two: Tasks →</a></div>
</div>
