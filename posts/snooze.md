# Never snooze a future
###### 2026 January 14<sup>th</sup> DRAFT

> Huh, that's confusing, because the task ought to be able to execute other
> futures in that case &mdash; so why are her connections stalling out without
> making progress?<br>
> \- [_Barbara battles buffered streams_][barbara]

[barbara]: https://rust-lang.github.io/wg-async/vision/submitted_stories/status_quo/barbara_battles_buffered_streams.html#-status-quo-stories-barbara-battles-buffered-streams

> Any time you have a single task polling multiple futures concurrently, be
> extremely careful that the task never stops polling a future that it
> previously started polling.<br>
> \- [_Futurelock_][futurelock]

[futurelock]: https://rfd.shared.oxide.computer/rfd/0609

> Buffer data, not code.<br>
> \- [boats][order]

[order]: https://without.boats/blog/futures-unordered/

When a future is ready to make progress, but it's not getting polled, I call
that "snoozing".[^starvation] Snoozing is to blame for a lot of hangs and
deadlocks in async Rust, including the recent ["Futurelock"][futurelock] case
study from the folks at Oxide. I'm going to argue that snoozing is almost
always a bug, that the tools and patterns that expose us to it should be
considered harmful, and that reliable and convenient replacements are possible.

[^starvation]: Snoozing is similar to "starvation", but starvation usually
    means that some other call to `poll` has blocked instead of returning
    quickly, which stops the executor from polling anything else while it
    waits. Snoozing is when the executor is running fine, but some futures
    still aren't getting polled.

[`FusedIterator`]: https://doc.rust-lang.org/std/iter/trait.FusedIterator.html

Before we dive in, I want to be clear that snoozing and cancellation are
different things. If a snoozed future eventually wakes up, then clearly it
wasn't cancelled. On the other hand, a cancelled future can also be snoozed, if
there's a gap between when it's last polled and when it's finally
dropped.[^define_cancellation] Cancellation bugs are a [big
topic][cancelling_async_rust] in async Rust, and it's good that we're talking
about them, but cancellation _itself_ isn't a bug. Snoozing _is_ a bug, and I
don't think we talk about it enough.

[^define_cancellation]: We often say that cancelling a future _means_ dropping
    it, but a future that's never going to be polled again has also arguably
    been cancelled, even if it hasn't yet been dropped. Which definition is
    better? I'm not sure, but if we agree that snoozing is a bug, then the
    difference only matters to buggy programs.

[cancelling_async_rust]: https://sunshowers.io/posts/cancelling-async-rust/

## Deadlocks

Snoozing can cause mysterious latencies and timeouts, but the clearest and most
dramatic snoozing bugs are deadlocks ("futurelocks"). Let's look at several
examples. Our test subject today will be `foo`, a toy function that takes a
private async lock and pretends to do some work:[^nothing_wrong]

[^nothing_wrong]: I want to emphasize that there's nothing wrong with `foo`. We
    could make examples like these with of any form of async blocking:
    semaphores, bounded channels, even [`OnceCell`]s. There's some [interesting
    advice in the Tokio docs][what_kind_of_mutex] about using regular locks
    instead of async locks as much as possible, and that's good advice, but
    consider that even `tokio::sync::mpsc` channels [use a semaphore
    internally][internally].

[what_kind_of_mutex]: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html#which-kind-of-mutex-should-you-use
[`OnceCell`]: https://docs.rs/tokio/latest/tokio/sync/struct.OnceCell.html
[internally]: https://github.com/tokio-rs/tokio/blob/0ec0a8546105b9f250f868b77e42c82809703aab/tokio/src/sync/mpsc/bounded.rs#L162

```rust
static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    tokio::time::sleep(Duration::from_millis(10)).await; // pretend work
}
```

As we go along, I want you to imagine that `foo` is buried three crates deep in
some dependency you've never heard of. When these things happen in real life,
the lock, the future that's holding it, and the mistake that snoozes that
future can all be far apart from each other.[^ghidra] With that in mind, here's
the minimal futurelock:

[^ghidra]: In the [original issue thread][gh9259] that inspired "Futurelock",
    they had to look at core dumps in [Ghidra] to find the bug. That's what we
    call ["type 2 fun"](https://essentialwilderness.com/type-1-2-and-3-fun/).

[gh9259]: https://github.com/oxidecomputer/omicron/issues/9259
[Ghidra]: https://github.com/NationalSecurityAgency/ghidra

```rust
LINK: Playground ## playground://snooze_playground/foo_poll.rs
let future1 = pin!(foo());
_ = poll!(future1);
foo().await; // Deadlock!
```

There are two calls to `foo` here. We get `future1` from the first call and
[`poll!`] it,[^poll_macro] which runs it to the point where it's acquired the
`LOCK` and started sleeping. Then we call `foo` again, it gives us another
future, and this time we `.await` it. In other words, we poll the second `foo`
future (and _only_ the second one) in a loop until it's finished.[^three_parts]
But it tries to take the same lock, and `future1` isn't going to release that
lock until we either poll `future1` again or drop it. Our loop will never do
either of those things &mdash; we've "snoozed" `future1` &mdash; so we're
deadlocked.

[^poll_macro]: The `poll!` macro calls [`Future::poll`] exactly once. In effect
    it's a more general version of [`Mutex::try_lock`] or [`Child::try_wait`],
    i.e. "try this potentially blocking operation, but if it would block, give
    up instead." We could also do the same thing with [`poll_fn`] or by
    [writing a `Future` "by hand"][poll_struct].

[`poll!`]: https://docs.rs/futures/latest/futures/macro.poll.html
[`Future::poll`]: https://doc.rust-lang.org/std/future/trait.Future.html#tymethod.poll
[`Mutex::try_lock`]: https://doc.rust-lang.org/std/sync/struct.Mutex.html#method.try_lock
[`Child::try_wait`]: https://doc.rust-lang.org/std/process/struct.Child.html#method.try_wait
[`poll_fn`]: playground://snooze_playground/foo_poll_fn.rs
[poll_struct]: playground://snooze_playground/foo_poll_struct.rs

[^three_parts]: There is a loop, but it's not really "inside" the `.await`.
    Instead, it's [in the runtime][block_on_loop]. This "inversion of control"
    is the very heart of async/await; this is why it's possible to run multiple
    futures concurrently on one thread. If you haven't seen the [`poll`] and
    [`Waker`] machinery that makes it all work, I recommend reading at least
    part one of [Async Rust in Three Parts][three_parts].

[block_on_loop]: https://github.com/tokio-rs/tokio/blob/tokio-1.49.0/tokio/src/runtime/park.rs#L283
[`poll`]: https://doc.rust-lang.org/std/future/trait.Future.html#tymethod.poll
[`Waker`]: https://doc.rust-lang.org/std/task/struct.Waker.html
[three_parts]: async_intro.html

That example is nice and short, but the `poll!` macro isn't common in real
programs. What you're more likely to see in practice is something like this
with [`select!`]:[^minimized]

[`select!`]: https://docs.rs/tokio/latest/tokio/macro.select.html

[^minimized]: The `select!` example in [_Futurelock_][futurelock] doesn't
    involve a loop, but if you pull up [the PR that fixed the
    bug][futurelock_pr], there's a loop just like this one. Looping is usually
    what forces us to select by reference, but where possible we can and should
    select by value, which drops cancelled futures promptly and [prevents this
    sort of deadlock][select_value].

[futurelock_pr]: https://github.com/oxidecomputer/omicron/pull/9268/changes#diff-26ed102e2389f81dd6551debec14f18eabf18cfa15b4e9321b20f61d3a925d12L516-L517
[select_value]: playground://snooze_playground/foo_select_value.rs

```rust
LINK: Playground ## playground://snooze_playground/foo_select_loop.rs
let mut future1 = pin!(foo());
loop {
    select! {
        _ = &mut future1 => break,
        // Do some periodic background work while `future1` is running.
        _ = tokio::time::sleep(Duration::from_millis(5)) => {
            foo().await; // Deadlock!
        }
    }
}
```

This loop is trying to to drive `future1` to completion, while waking up every
so often to do some background work. The `select!` macro polls both `&mut
future1` and a [`Sleep`] future until one of them is ready, then it drops both
of them and runs the `=>` body of the winner.[^output] The loop creates a new
`Sleep` future each time around, but it doesn't want to restart `foo`, so it
selects on `future1` _by reference_. But that only keeps `future1` alive; it
doesn't mean that it keeps getting polled. The intent is to poll `future1`
again in the next loop iteration, but we're snoozing it during the background
work, which happens to include another call to `foo`, so we're deadlocked
again.

[`Sleep`]: https://docs.rs/tokio/latest/tokio/time/struct.Sleep.html

[^output]: If the winner had useful output, we could capture it with a variable
    name (or in general any "pattern") to the left of the `=` sign. Both
    outputs here are `()`, so we use `_` to ignore them. This is the same way
    `_` works in assignments, function arguments, and `match` arms.

We can also provoke this deadlock by selecting on a [stream]:

[stream]: https://tokio.rs/tokio/tutorial/streams

```rust
LINK: Playground ## playground://snooze_playground/foo_select_streams.rs
let mut stream = pin!(stream::once(foo()));
select! {
    _ = stream.next() => {}
    _ = tokio::time::sleep(Duration::from_millis(5)) => {}
}
foo().await; // Deadlock!
```

In this case the [`Next`] future isn't a reference, and `select!` does drop it,
but we've managed to snooze the stream itself and the `foo` future inside of
it.[^stream_snoozing]

[`Next`]: https://docs.rs/futures/latest/futures/stream/struct.Next.html

[^stream_snoozing]: The problem of snoozing streams is especially subtle. It's
    normal and expected to call [`next`] to pull an item from the stream, and
    then to not do that again for a while. That's just iteration, not snoozing.
    In particular, when [`poll_next`] returns `Ready(Some(_))`, it doesn't
    register a wakeup. Wakeups are only registered when polling returns
    `Pending`. In generator terms (using the nightly-only [`gen`, `async gen`,
    and `yield` keywords][async_gen]) returning an item is a _yield point_.
    Note that there's no way for a stream to somehow "inject" a yield point
    into `foo`'s critical section. (Other than by committing a snoozing crime
    internally, which isn't the case here, though see `FuturesUnordered`
    below). But in this example, we haven't paused the stream at a yield point.
    Instead, we've paused it at an _await point_, which _has_ registered a
    wakeup and which _does_ expect to get polled promptly when it's ready.
    That's why this example still counts as snoozing. When we start a call to
    `next`, or in general when `poll_next` returns `Pending`, we either need to
    keep driving the stream until it yields an item, or else we need to drop
    _the whole stream_. (TODO: This rules out selecting on channel receivers,
    which probably goes to far. Maybe we can make an exception for `Unpin`
    types? Or maybe channel receivers should expose non-`Stream` APIs. I'm not
    sure.)

[`next`]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.next
[`poll_next`]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html#tymethod.poll_next
[async_gen]: playground://snooze_playground/async_gen_example.rs?version=nightly

Speaking of streams, another category of futurelocks comes from ["buffered"]
streams:

["buffered"]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.buffered

```rust
LINK: Playground ## playground://snooze_playground/foo_buffered.rs
futures::stream::iter([foo(), foo()])
    .buffered(2)
    .for_each(|_| foo()) // Deadlock!
    .await;
```

Here the buffer starts polling both of its `foo` futures concurrently. When the
first one finishes, control passes to the `for_each` closure. While that
closure is running, the other `foo` in the buffer is snoozed.[^fair]

[^fair]: In this case the second buffered `foo` doesn't actually advance to the
    point where it acquires the `LOCK`. But we still get a reliable deadlock
    here, because Tokio's `Mutex` is "fair". When `Mutex::lock` blocks waiting
    for the `Mutex` to be released, it takes a "place in line", and other
    callers can't jump ahead unless it's cancelled. To [make this example work
    with an unfair mutex][unfair], we could add a 1 ms sleep in `foo` after the
    critical section.

[unfair]: playground://snooze_playground/foo_buffered_unfair.rs

Buffered streams are a wrapper around either [`FuturesOrdered`] or
[`FuturesUnordered`], and we can hit the same deadlock by looping over either
of those directly:[^stream_fault]

[`FuturesOrdered`]: https://docs.rs/futures/latest/futures/stream/struct.FuturesOrdered.html
[`FuturesUnordered`]: https://docs.rs/futures/latest/futures/stream/struct.FuturesUnordered.html

[^stream_fault]: Contrast this example with the `stream::once` example above.
    There we were "at fault" for snoozing the stream in between yield points,
    but here our program faithfully drives `FuturesUnordered` to a yield point,
    and it still snoozes the other `foo` internally.

```rust
LINK: Playground ## playground://snooze_playground/foo_unordered.rs
let mut futures = FuturesUnordered::new();
futures.push(foo());
futures.push(foo());
while let Some(_) = futures.next().await {
    foo().await; // Deadlock!
}
```

Invisible deadlocks are bad, but what's worse is that it's hard to describe
what exactly these examples are doing wrong.[^chilling] Is `foo` broken?[^no]
Are `select!` and buffered streams broken? Are these programs "holding them
wrong"?

[^chilling]: "There's no one abstraction, construct, or programming pattern we
    can point to here and say 'never do this'."<br>
    \- [_Futurelock_][futurelock]

[^no]: No, `foo` is not broken.

I want to answer those questions with a different question: Why don't we have
these problems when we use regular locks and threads?

## Threads

> How many times does<br>
> it have to be said: Never<br>
> call `TerminateThread`.<br>
> \- [Larry Osterman][oldnewthing]

[oldnewthing]: https://devblogs.microsoft.com/oldnewthing/20150814-00/?p=91811

Let's think about a regular, non-async version of `foo`:

```rust
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn foo() {
    let _guard = LOCK.lock().unwrap();
    thread::sleep(Duration::from_millis(10));
}
```

Again there's only one lock here, and this non-async `foo` always releases it
after 10 ms. It should be _impossible_ for this function to participate in a
deadlock. Right?

Well...sort of. I don't think we'd ever _blame_ `foo` for a deadlock. But it is
possible to deadlock with `foo`, if we somehow kill the thread it's running on.
The Windows `TerminateThread` function [warns us about this][terminatethread]:
"If the target thread owns a critical section, the critical section will not be
released."[^dangerous] The classic cause of these problems on Unix is `fork`,
which copies the whole address space of a process but only one of its running
threads.[^fork_example][^fork] There's nothing a function like `foo` can
realistically do to protect itself from this,[^cleanup] so instead the general
rule is "Don't kill threads."

[terminatethread]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminatethread

[^dangerous]: The docs also call it "a dangerous function that should only be
    used in the most extreme cases". They don't elaborate on what counts as an
    extreme case.

[^fork_example]: [Playground example][fork_example]

[fork_example]: playground://snooze_playground/foo_fork.rs

[^fork]: "Programming guides advise not using fork in a multithreaded process,
    or calling exec immediately afterwards. POSIX only guarantees that a small
    list of 'async-signal-safe' functions can be used between fork and exec,
    notably excluding `malloc()` and anything else in standard libraries that
    may allocate memory or acquire locks. Real multithreaded programs that fork
    are plagued by bugs arising from the practice. It is hard to imagine a new
    proposed syscall with these properties being accepted by any sane kernel
    maintainer." - [_A `fork()` in the road_][fork_in_the_road]

[fork_in_the_road]: https://www.microsoft.com/en-us/research/wp-content/uploads/2019/04/fork-hotos19.pdf

[^cleanup]: On Unix it's technically possible to do cleanup in these situations
    with hooks like [`pthread_atfork`] and [`pthread_cleanup_push`], but it's
    not practical. Preventing memory leaks, for example, would mean registering
    callbacks for every single allocation. Even worse, we'd need to do that
    _atomically_, so that cancellation or forking can't occur in between an
    allocation and its registration. We can postpone cancellations with
    [`pthread_setcancelstate`], but forking has no equivalent. And there are no
    cleanup hooks for `TerminateThread` on Windows.

[`pthread_atfork`]: https://man7.org/linux/man-pages/man3/pthread_atfork.3.html
[`pthread_cleanup_push`]: https://man7.org/linux/man-pages/man3/pthread_cleanup_push.3.html
[`pthread_setcancelstate`]: https://man7.org/linux/man-pages/man3/pthread_setcancelstate.3.html

Given the historical tire fire that is thread cancellation, it's remarkable
that cancelling futures works as well as it does. The crucial difference is
that Rust knows how to `drop` a future and clean up the resources it owns,
particularly the lock guards.[^unaliased] The OS can clean up a whole process
when it exits, but it can't tell which threads within a process own what, and
it can only trust that they clean up after themselves.

[^unaliased]: Related to that, Rust knows that no part of an object is borrowed
    at the point where we `drop` it.

Another way to deadlock with the non-async `foo` is to _pause_ the thread it's
running on. The Windows docs [warn us about this too][suspendthread]: "Calling
`SuspendThread` on a thread that owns a synchronization object, such as a mutex
or critical section, can lead to a deadlock if the calling thread tries to
obtain a synchronization object owned by a suspended thread." The classic cause
of these problems Unix is signal handlers, which hijack a thread whenever they
run.[^signal_example][^signalfd][^signal_safe] Again there's nothing `foo` can
realistically do to protect itself from this, so the general rule is "Don't
pause threads."

[suspendthread]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-suspendthread

[^signal_example]: [Playground example][signal_example]

[signal_example]: playground://snooze_playground/foo_signal.rs

[^signalfd]: "If you register a signal handler, it's called in the middle of
    whatever code you happen to be running. This sets up some very onerous
    restrictions on what a signal handler can do: it can't assume that any
    locks are unlocked, any complex data structures are in a reliable state,
    etc. The restrictions are *stronger* than the restrictions on thread-safe
    code, since the signal handler interrupts and *stops* the original code
    from running. So, for instance, it can't even wait on a lock, because the
    code that's holding the lock is paused until the signal handler completes.
    This means that a lot of convenient functions, including the `stdio`
    functions, `malloc`, etc., are unusable from a signal handler, because they
    take locks internally." - [_signalfd is
    useless_](https://ldpreload.com/blog/signalfd-is-useless)

[^signal_safe]: In fact this is where `fork`'s list of "async-signal-safe"
    functions comes from. The rules for what you can do after `fork` are mostly
    the same as what you can do in a signal handler.

In contrast to cancellation, pausing ("snoozing") a future is no better than
pausing a thread. Async locks aren't as common as regular ones, so futurelock
isn't as well understood as the classic problems with `fork` and signal
handlers, but it's fundamentally the same problem. Whenever one thread or
future waits for another in any way &mdash; whether that's with locks,
channels, [`join`], or anything &mdash; we need to know that the graph of
who's-waiting-on-whom doesn't contain cycles. If "forces beyond our control"
can add edges to that graph, then there's no way for us to write correct
concurrent programs.

[`join`]: https://doc.rust-lang.org/std/thread/struct.JoinHandle.html#method.join

So, were the async examples in the last section "holding it wrong"? Maybe in
the same sense that programs that call `TerminateThread` are holding it wrong.
[The only right way to hold it is not to hold it.][not_to_play] It arguably
shouldn't exist.[^shouldnt_exist] No async runtime has a `pause_task` function,
either, because the docs would just say "Don't use this". And yet that's what
we have, implicitly, when we use `select!`-by-reference or buffered streams
today.

[not_to_play]: https://youtu.be/MpmGXeAtWUw?t=90

[^shouldnt_exist]: "The original designers felt strongly that no such function
    should exist because there was no safe way to terminate a thread, and
    there's no point having a function that cannot be called safely."<br>
    \- [Raymond Chen][oldnewthing]

## What is to be done: `select!`

Using `select!` with owned futures is no problem,[^exception] as long as we're
ok with cancellation, because `select!` drops all its "scrutinee" futures
promptly. Using `select!` with references is what we really need to avoid.
Unfortunately, that's easier said than done.

[^exception]: We saw an exception above: `stream.next()` returned a future, but
    selecting on it still caused a deadlock. We'll get to that.

Running each future on its own task with [`tokio::spawn`][spawn] is one way to
prevent snoozing &mdash; like threads, tasks have a "life of their own" &mdash;
but it comes with a `'static` bound that clashes with any sort of
borrowing.[^arc_mutex] The [`moro`] crate provides a non-`'static` task
spawning API similar to [`std::thread::scope`], and it can solve some of these
problems.[^moro] But Niko Matsakis' ["case study of pub-sub in
mini-redis"][mini_redis] illustrates how `select!` is more flexible than scoped
tasks: `select!` macro-expands into a `match`, and different `match` arms are
allowed to mutate the same variables.[^mutate_scrutinees] Lots of real projects
take advantage of that.

[^arc_mutex]: The most common way to fix these errors is by liberally applying
    `Arc<Mutex<_>>`, but that's annoying at best, and it can require a large
    refactoring if the borrow was coming from the caller.

[^moro]: `moro` runs all its tasks on the same thread (i.e. within the current
    task), which avoids the ["Scoped Task Trilemma"][trilemma]. Running scoped
    tasks on different threads safely is a major open problem in async Rust.

[^mutate_scrutinees]: In fact, if we're selecting on a reference to a future or
    a stream, the arm bodies can even mutate that future or stream itself,
    because the reference gets dropped before the `match`. In other words, the
    fact that scrutinees get snoozed is visible to the borrow checker, in a way
    that real code in the wild depends on! Supporting these patterns without
    any risk of snoozing is [very complicated][mutable_access].

[mutable_access]: https://github.com/oconnor663/join_me_maybe#mutable-access-to-futures-and-streams

[spawn]: https://docs.rs/tokio/latest/tokio/task/fn.spawn.html
[`moro`]: https://github.com/nikomatsakis/moro
[`std::thread::scope`]: https://doc.rust-lang.org/std/thread/fn.scope.html
[trilemma]: https://without.boats/blog/the-scoped-task-trilemma/
[mini_redis]: https://smallcultfollowing.com/babysteps/blog/2022/06/13/async-cancellation-a-case-study-of-pub-sub-in-mini-redis/

I have an experimental crate aimed at addressing this: [`join_me_maybe`]. It
provides a snooze-free `join!` macro with some `select!`-like features. Here's
how it replaces the `select!` loop above:[^alternatives]

[^alternatives]: `join_me_maybe` has several ways to express this. Apart from
    [the `maybe` keyword][cancel_maybe] shown here, you can also [`.cancel()` a
    labeled arm][cancel_method] or [`return` from the calling
    function][cancel_return].

[cancel_maybe]: https://github.com/oconnor663/join_me_maybe#maybe-cancellation
[cancel_method]: https://github.com/oconnor663/join_me_maybe#label-and-cancel
[cancel_return]: https://github.com/oconnor663/join_me_maybe#arm-bodies-with-

```rust
join_me_maybe::join!(
    foo(),
    // Do some periodic background work while the first `foo` is
    // running. `join!` runs both arms concurrently, but the `maybe`
    // keyword means it doesn't wait for this arm to finish.
    maybe async {
        loop {
            tokio::time::sleep(Duration::from_millis(5)).await;
            foo().await;
        }
    }
);
```

[`join_me_maybe`]: https://github.com/oconnor663/join_me_maybe/

Like most "join" idioms today, this `join!` macro owns the futures that it
polls, and there's no window for the caller to snooze
anything.[^join_reference] It needs some real-world feedback before I can
recommend it for general use, but it can currently tackle both [the original
"Futurelock" `select!`][join_me_maybe_omicron] and [the `select!` that
frustrated `moro` in mini-redis][join_me_maybe_mini_redis]. There's a wide open
design space for concurrency patterns like this, and I think there's also room
for [new language features] that could allow for even more concurrency than
`select!` supports today.

[join_me_maybe_mini_redis]: https://github.com/oconnor663/mini-redis/pull/1
[join_me_maybe_omicron]: https://github.com/oconnor663/omicron/pull/1
[new language features]: https://github.com/oconnor663/join_me_maybe#help-needed-from-the-compiler

[^join_reference]: Or more accurately, it _can_ own them, and there's no
    particular reason for us to go out of our way to `pin!` a `foo` future and
    pass it in by reference. But that's still possible, and we can still cause
    snoozing by doing it. Macros like `join_me_maybe::join!` let us express
    more with owned futures, but banning await-by-reference is a separate
    question. More on that below.

## What is to be done?

> This method is cancel safe.<br>
> \- [`tokio_stream::StreamExt::next`](https://docs.rs/tokio-stream/latest/tokio_stream/trait.StreamExt.html#method.next)

\[work in progress\]

replacing `select!`-by-reference:

- <https://github.com/oconnor663/join_me_maybe>

fixing streams:

- <https://github.com/oconnor663/roughage>
- <https://without.boats/blog/poll-progress>
