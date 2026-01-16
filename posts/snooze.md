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
    means that a call to `poll` is blocking instead of returning quickly, which
    stops the executor from polling anything else while it waits. Snoozing is
    when the executor is running fine, but some futures still aren't getting
    polled.

[`FusedIterator`]: https://doc.rust-lang.org/std/iter/trait.FusedIterator.html

Before we dive in, I want to be clear that snoozing and cancellation are
different things. If a snoozed future eventually wakes up, then clearly it
wasn't cancelled. On the other hand, a cancelled future can also be snoozed, if
there's a gap between when it's last polled and when it's finally
dropped.[^define_cancellation] Cancellation bugs are a [big topic] in async
Rust, and it's good that we're talking about them, but cancellation _itself_
isn't a bug. Snoozing _is_ a bug, and I don't think we talk about it enough.

[^define_cancellation]: We often say that cancelling a future _means_ dropping
    it, but a future that's never going to be polled again has also arguably
    been cancelled, even if it hasn't yet been dropped. Which definition is
    better? I'm not sure, but if we agree that snoozing is a bug, then the
    difference only matters to buggy programs.

[big topic]: https://sunshowers.io/posts/cancelling-async-rust/

## Deadlocks

Snoozing can cause mysterious latencies and timeouts, but the clearest and most
dramatic snoozing bugs are deadlocks ("futurelocks"). Let's look at several
examples. Our test subject today is `foo`, a toy function that takes a private
async lock and pretends to do some work:[^nothing_wrong]

[^nothing_wrong]: I want to emphasize that there's nothing wrong with `foo`. We
    could make examples like these with of any form of async blocking:
    semaphors, bounded channels, even [`OnceCell`]s. There's some [interesting
    advice in the Tokio docs][what_kind_of_mutex] about using regular locks
    instead of async locks as much as possible, and that's good advice, but
    consider that even `tokio::sync::mpsc` channels [use a semaphor
    internally][internally].

[what_kind_of_mutex]: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html#which-kind-of-mutex-should-you-use
[`OnceCell`]: https://docs.rs/tokio/latest/tokio/sync/struct.OnceCell.html
[internally]: https://github.com/tokio-rs/tokio/blob/0ec0a8546105b9f250f868b77e42c82809703aab/tokio/src/sync/mpsc/bounded.rs#L162

```rust
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

static LOCK: Mutex<()> = Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    sleep(Duration::from_millis(10)).await; // very important work
}
```

As we go along, I want you to imagine that `foo` is buried three crates deep in
some dependency you've never heard of. In real life the lock, the future that's
holding it, and the mistake that snoozes that future can be far apart from each
other.[^ghidra] With that in mind, here's the minimal futurelock:

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
future, and this time we `.await` it. In other words, we're polling the second
`foo` future (_only_ the second one) in a _loop_ until it's
finished.[^three_parts] But it tries to take the same lock, and `future1` isn't
going to release that lock until we either poll `future1` again or drop it. Our
loop will never do either of those things &mdash; we've implicitly "snoozed"
`future1` &mdash; so we're deadlocked.

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
    is the very heart of async/await; that's why it's possible to run multiple
    futures concurrently on one thread. If you haven't seen the [`poll`] and
    [`Waker`] machinery that makes all this work, I recommend reading at least
    part one of [Async Rust in Three Parts][three_parts].

[block_on_loop]: https://github.com/tokio-rs/tokio/blob/tokio-1.49.0/tokio/src/runtime/park.rs#L283
[`poll`]: https://doc.rust-lang.org/std/future/trait.Future.html#tymethod.poll
[`Waker`]: https://doc.rust-lang.org/std/task/struct.Waker.html
[three_parts]: async_intro.html

You probably won't see anyone use `poll!` quite like that in the wild. What
you'll see is effectively the same thing, but with
[`select!`](https://docs.rs/tokio/latest/tokio/macro.select.html):[^minimized]

[^minimized]: The `select!` example in [_Futurelock_][futurelock] doesn't
    involve a loop, but if you pull up [the PR that fixed the
    bug][futurelock_pr], there's a loop just like this one. Looping is usually
    what forces us to select by reference, but where possible we can and should
    select _by value_, which drops futures promptly and [prevents this sort of
    deadlock][select_value].

[futurelock_pr]: https://github.com/oxidecomputer/omicron/pull/9268/changes#diff-26ed102e2389f81dd6551debec14f18eabf18cfa15b4e9321b20f61d3a925d12L516-L517
[select_value]: playground://snooze_playground/foo_select_value.rs

```rust
LINK: Playground ## playground://snooze_playground/foo_select_loop.rs
let mut future1 = pin!(foo());
loop {
    select! {
        _ = &mut future1 => break,
        // Do some periodic background work while `future1` is running.
        _ = sleep(Duration::from_millis(5)) => foo().await, // Deadlock!
    }
}
```

This loop is trying to to run `future1` until it's finished, while waking up
every so often to do some background work. The `select!` macro polls its
arguments until one of them is ready,[^output] then it drops both of them and
runs the `=>` body of the winner. However, because `future1` needs to stay
alive for the whole loop, this `select!` is polling it _by reference_, and
dropping that reference has no effect. Instead we snooze `future1` during the
background work, which happens to include another call to `foo`, so we're
deadlocked again.

[`Sleep`]: https://docs.rs/tokio/latest/tokio/time/struct.Sleep.html

[^output]: If we cared about these futures' outputs, we could capture them with
    a variable name (or in general any "pattern") to the left of the `=` sign.
    But these futures both output `()`, so we use `_` to ignore/drop those, the
    same way `_` works in assignments, function arguments, and `match` arms.

We can also provoke this deadlock by selecting on a [stream]:

[stream]: https://tokio.rs/tokio/tutorial/streams

```rust
LINK: Playground ## playground://snooze_playground/foo_select_streams.rs
let mut stream = pin!(stream::once(foo()));
select! {
    _ = stream.next() => {}
    _ = sleep(Duration::from_millis(5)) => {}
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
    Instead, we've paused it at an _await point_, which has registered a wakeup
    and which does expect to get polled promptly when it's ready. That's why
    this example still counts as snoozing. When we start a call to `next`, or
    in general when `poll_next` returns `Pending`, we either need to keep
    driving the stream until it yields an item, or else we need to drop the
    _whole stream_. (TODO: This rules out selecting on channel receivers, which
    probably goes to far. Maybe we can make an exception for `Unpin` types? Or
    maybe channel receivers should expose non-`Stream` APIs. I'm not sure.)

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
[`FuturesUnordered`], and we can trigger the same deadlock by looping over
either of those directly:[^stream_fault]

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

Invisible deadlocks are bad, but what's worse is that it's hard to pinpoint
what exactly these examples are doing wrong.[^chilling] Is `foo` broken?[^no] Are
`select!` and buffered streams broken? Are these programs "holding them wrong"?

[^chilling]: "There's no one abstraction, construct, or programming pattern we
    can point to here and say 'never do this'."<br>
    \- [_Futurelock_][futurelock]

[^no]: No, `foo` is not broken.

Let's start answering those questions with a different question: Why don't we
have these problems when we use regular threads?

## Threads

> How many times does<br>
> it have to be said: Never<br>
> call `TerminateThread`.<br>
> \- [Larry Osterman][oldnewthing]

[oldnewthing]: https://devblogs.microsoft.com/oldnewthing/20150814-00/?p=91811

In fact, we _do_ have these problems with threads, if we try to cancel them.
The Windows `TerminateThread` function [warns us about this][terminatethread]:
"If the target thread owns a critical section, the critical section will not be
released."[^dangerous] The classic source of cancellation deadlocks on Unix is
`fork`, which copies the whole address space of the parent process but only one
of its running threads.[^fork]

[terminatethread]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminatethread

[^dangerous]: The docs also call it "a dangerous function that should only be
    used in the most extreme cases". They don't elaborate on what counts as an
    extreme case.

[^fork]: "Programming guides advise not using fork in a multithreaded process,
    or calling exec immediately afterwards. POSIX only guarantees that a small
    list of 'async-signal-safe' functions can be used between fork and exec,
    notably excluding `malloc()` and anything else in standard libraries that
    may allocate memory or acquire locks. Real multithreaded programs that fork
    are plagued by bugs arising from the practice. It is hard to imagine a new
    proposed syscall with these properties being accepted by any sane kernel
    maintainer." - [_A `fork()` in the road_][fork_in_the_road]

[fork_in_the_road]: https://www.microsoft.com/en-us/research/wp-content/uploads/2019/04/fork-hotos19.pdf

Given the historical tire fire that is thread cancellation, it's remarkable
that cancelling futures works as well as it does. The crucial difference is
that Rust knows how to `drop` a future and clean up the resources it owns,
particularly the lock guards.[^unaliased] The OS can clean up a process when it
exits, but it can't tell which threads within a process own what, and it can
only hope they clean up after themselves.

[^unaliased]: Related to that, Rust knows that no part of an object is borrowed
    at the point where we `drop` it.

We also have these problems if we _pause_ threads. The Windows docs [warn us
about this too][suspendthread]: "Calling `SuspendThread` on a thread that owns
a synchronization object, such as a mutex or critical section, can lead to a
deadlock if the calling thread tries to obtain a synchronization object owned
by a suspended thread." The classic source of pausing deadlocks on Unix is
signal handlers, which hijack a thread whenever they
run.[^signalfd][^signal_safe]

[suspendthread]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-suspendthread

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
    the same as the rules for what you can do in a signal handler.

Cancelling futures works reasonably well, but snoozing a future is just as
broken as pausing a thread, because we're not letting the future clean up after
itself, and we're also not letting it `drop`. Outside of carefully controlled
circumstances, we have to assume that snoozing any future or stream could
deadlock the whole process. Async locks aren't as ubiquitous as the locks in
`malloc` or `std::io`, so it's harder to notice this problem today, but it's
fundamentally the same problem.

## What is to be done?

> This method is cancel safe.<br>
> \- [`tokio_stream::StreamExt::next`](https://docs.rs/tokio-stream/latest/tokio_stream/trait.StreamExt.html#method.next)

\[work in progress\]

replacing `select!`-by-reference:

- <https://github.com/oconnor663/join_me_maybe>

fixing streams:

- <https://github.com/oconnor663/roughage>
- <https://without.boats/blog/poll-progress>
