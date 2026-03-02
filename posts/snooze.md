# Never snooze a future
###### 2026 March 2<sup>nd</sup>

> Huh, that's confusing, because the task ought to be able to execute other
> futures in that case &mdash; so why are her connections stalling out without
> making progress?<br>
> \- [_Barbara battles buffered streams_][barbara]

[barbara]: https://rust-lang.github.io/wg-async/vision/submitted_stories/status_quo/barbara_battles_buffered_streams.html

When a future is ready to make progress, but it's not getting polled, I call
that "snoozing". Snoozing is to blame for a lot of hangs and deadlocks in async
Rust, including the recent ["Futurelock"][futurelock] case study from the folks
at Oxide. I'm going to argue that snoozing is almost always a bug, that the
tools and patterns that expose us to it should be considered harmful, and that
reliable and convenient replacements are possible.

Before we dive in, I want to be clear that snoozing and cancellation are
different things.[^starvation] If a snoozed future eventually wakes up, then
clearly it wasn't cancelled. On the other hand, a cancelled future can also be
snoozed, if there's a gap between when it's last polled and when it's finally
dropped.[^define_cancellation] Cancellation bugs are a [big
topic][cancelling_async_rust] in async Rust, and it's good that we're talking
about them, but cancellation _itself_ isn't a bug. Snoozing _is_ a bug, and I
don't think we talk about it enough.

[^starvation]: Snoozing and starvation are also different things. Starvation is
    when something is hogging the executor and getting in the way of polling
    other futures. Snoozing is when everything runs smoothly to idle, but some
    future that requested a wakeup still doesn't get polled.

[^define_cancellation]: We often say that cancelling a future _means_ dropping
    it, but a future that's never going to be polled again has also arguably
    been cancelled, even if it hasn't yet been dropped. Which definition is
    better? I'm not sure, but if we agree that snoozing is a bug, then the
    difference only matters to buggy programs.

[cancelling_async_rust]: https://sunshowers.io/posts/cancelling-async-rust/

## Deadlocks

> Any time you have a single task polling multiple futures concurrently, be
> extremely careful that the task never stops polling a future that it
> previously started polling.<br>
> \- [_Futurelock_][futurelock]

[futurelock]: https://rfd.shared.oxide.computer/rfd/0609

Snoozing can cause mysterious latencies and timeouts, but the clearest and most
dramatic snoozing bugs are deadlocks ("futurelocks"). Let's look at several
examples. Our test subject today will be `foo`, a toy function that takes a
private async lock and pretends to do some work:[^nothing_wrong][^private]

[^nothing_wrong]: There's nothing wrong with `foo`. We could make examples like
    these with any form of async waiting: semaphores, bounded channels, even
    [`OnceCell`]s. There's some [interesting advice in the Tokio
    docs][what_kind_of_mutex] about using regular locks instead of async locks
    as much as possible, and that's good advice, but consider that even
    `tokio::sync::mpsc` channels [use a semaphore internally][internally].

[what_kind_of_mutex]: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html#which-kind-of-mutex-should-you-use
[`OnceCell`]: https://docs.rs/tokio/latest/tokio/sync/struct.OnceCell.html
[internally]: https://github.com/tokio-rs/tokio/blob/0ec0a8546105b9f250f868b77e42c82809703aab/tokio/src/sync/mpsc/bounded.rs#L162

[^private]: Nothing besides `foo` is going to touch `LOCK`, so it would be
    cleaner to move it into `foo`'s body. I'm keeping it this way because not
    everyone has seen function-local `static`s before, and they can be
    confusing the first time you see them.

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
    they had to look at core dumps in [Ghidra] to narrow down the bug. That's
    what we call ["type 2
    fun"](https://essentialwilderness.com/type-1-2-and-3-fun/).

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
future in a loop until it's finished.[^three_parts] But it tries to take the
same lock, and `future1` isn't going to release that lock until we either poll
`future1` again or drop it. Our loop isn't going to do either of those things
&mdash; we've "snoozed" `future1` &mdash; so we're deadlocked.

[^poll_macro]: The `poll!` macro calls [`Future::poll`] exactly once. It's
    effectively a more general version of [`Mutex::try_lock`] or
    [`Child::try_wait`], i.e. "try this potentially blocking operation, but if
    it does need to block, give up instead." We could also do the same thing
    with [`poll_fn`] or by [writing a `Future` "by hand"][poll_struct].

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
    sort of deadlock][select_value]. More on this below.

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

This loop is trying to drive `future1` to completion, while waking up every
so often to do some background work. The `select!` macro polls both `&mut
future1` and a [`Sleep`] future until one of them is ready, then it drops both
of them and runs the `=>` body of the winner.[^output] The loop creates a new
`Sleep` future each time around, but it doesn't want to restart `foo`, so it
selects on `future1` _by reference_. But that only keeps `future1` alive; it
doesn't mean that it keeps getting polled. The intent is to poll `future1`
again in the next loop iteration, but we snooze it during the background work,
which happens to include another call to `foo`, and we're deadlocked again.

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

In this case the [`stream.next()`][next] future is actually a value, not a
reference, and it does get dropped after the `sleep` finishes. But it
_contains_ a reference to the stream, and we still end up snoozing the `foo`
future inside that stream after we cancel `next`.[^stream_snoozing]

[next]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.next

[^stream_snoozing]: What counts as snoozing a stream is a bit tricky, and it's
    also possible that [the low-level API contract could change][poll_progress]
    before it's finally stabilized. (Even the name is uncertain: today we use
    the [`Stream`] trait from the `futures` crate, but the nightly-only version
    in the standard library is called [`AsyncIterator`].) The key detail is
    that while [`Future::poll`] represents two possible states,
    [`Stream::poll_next`][poll_next] represents _three_. Futures and streams
    both return `Ready(_)` and `Ready(None)` respectively when they're
    finished. And they both return `Pending` when they've registered a wakeup
    and need to be polled again later. In async function terms that's an "await
    point", and that's where snoozing can happen. But streams have a third
    state: `Ready(Some(_))` yields a value from the stream, which means the
    stream isn't finished, but at the same time it (typically, currently) has
    _not_ registered a wakeup. This is a "yield point", not an await point, and
    it corresponds to the `yield` keyword in the [nightly-only `gen` / `async
    gen` syntax][async_gen]. Cancelling a call to `.next()` leaves the stream
    (and any futures it might contain) at an arbitrary await point, which is
    how we snooze `foo` and get a deadlock in this example. But completing a
    call to `.next()` leaves the stream at a yield point, not an await point,
    and we probably don't want to count that as "snoozing the stream". More on
    this below.

[poll_progress]: https://without.boats/blog/poll-progress/
[`Stream`]: https://docs.rs/futures/latest/futures/prelude/trait.Stream.html
[`AsyncIterator`]: https://doc.rust-lang.org/std/async_iter/trait.AsyncIterator.html
[poll_next]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html#tymethod.poll_next
[async_gen]: playground://snooze_playground/async_gen_example.rs?version=nightly

Speaking of streams, another category of futurelocks comes from
[`buffered`][buffered] streams:[^buffered]

[buffered]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.buffered

[^buffered]: Like most of the methods on [`StreamExt`], [`buffered`][buffered]
    takes a stream of inputs and adapts it into another stream. But unlike most
    of the other methods, `buffered` assumes that the input items _are
    themselves futures_, and it awaits them and collects their outputs
    internally. This [`iter`] stream's `Item` type is `foo` futures, which is
    totally different from the [`once`] stream's `Item` type in the previous
    example, `()`.

[`StreamExt`]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html
[`iter`]: https://docs.rs/futures/latest/futures/stream/fn.iter.html
[`once`]: https://docs.rs/futures/latest/futures/stream/fn.once.html

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
    and it still snoozes the other `foo` internally. I think we'll ultimately
    need different fixes for these different cases. More on this below.

```rust
LINK: Playground ## playground://snooze_playground/foo_unordered.rs
let mut futures = FuturesUnordered::new();
futures.push(foo());
futures.push(foo());
while let Some(_) = futures.next().await {
    foo().await; // Deadlock!
}
```

Deadlocks are bad, but what's worse is that it's hard to pinpoint exactly what
these examples have done wrong.[^chilling] Is `foo` broken? Are `select!`
and buffered streams broken? Are these programs "holding them wrong"?

[^chilling]: "There's no one abstraction, construct, or programming pattern we
    can point to here and say 'never do this'."<br>
    \- [_Futurelock_][futurelock]

Rather than jumping straight into answering those questions,[^answers] I want
to ask an entirely different question: Why don't we have deadlocks like these
when we use regular locks and threads?

[^answers]: No, no, yes, and it's complicated.

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

Assuming that this `foo` is the only function that touches this `LOCK`, is it
even _possible_ for there to be a deadlock here?

The short, reasonable answer is no. But the long, pedantic answer is yes, if
we're willing to break a [long-standing rule][ancient_wisdom] of systems
programming and kill the thread that `foo` is running on. The Windows
`TerminateThread` function [warns us about this][terminatethread]: "If the
target thread owns a critical section, the critical section will not be
released."[^dangerous][^shouldnt_exist] The classic cause of these problems on
Unix is `fork`, which copies the whole address space of a process but only one
of its running threads.[^fork_example][^fork] There's nothing a function like
`foo` can realistically do to protect itself from this,[^cleanup] so instead
the general rule is "Never kill a thread."

[ancient_wisdom]: https://users.rust-lang.org/t/pthread-cancel-undefined-behavior/38477/3
[terminatethread]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminatethread

[^dangerous]: The docs also call it "a dangerous function that should only be
    used in the most extreme cases". They don't elaborate on what counts as an
    extreme case.

[^shouldnt_exist]: "The original designers felt strongly that no such function
    should exist because there was no safe way to terminate a thread, and
    there's no point having a function that cannot be called safely." \-
    [Raymond Chen][oldnewthing]

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

[^cleanup]: On Unix it's possible to do cleanup in these situations with
    [`pthread_atfork`] and [`pthread_cleanup_push`], but it's not practical.
    Preventing memory leaks would mean registering callbacks for every single
    allocation, and we'd need to do that atomically somehow, so that
    cancellation or forking can't occur in between an allocation and its
    registration. (We can postpone cancellations with
    [`pthread_setcancelstate`], but forking has no equivalent.) We'd also need
    to figure out how all of this interacts with move semantics, which would
    presumably require changes to the compiler itself.

[`pthread_atfork`]: https://man7.org/linux/man-pages/man3/pthread_atfork.3.html
[`pthread_cleanup_push`]: https://man7.org/linux/man-pages/man3/pthread_cleanup_push.3.html
[`pthread_setcancelstate`]: https://man7.org/linux/man-pages/man3/pthread_setcancelstate.3.html

Given the historical tire fire that is thread cancellation, it's remarkable
that cancelling futures works as well as it does. The crucial difference is
that Rust knows how to `drop` a future and clean up the resources it owns,
particularly the lock guards.[^unaliased] The OS can clean up a whole process
when it exits, but until then it doesn't know which thread owns what.

[^unaliased]: Rust also knows that no part of an object is borrowed at the
    point where we `drop` it.

It's also possible to deadlock this version of `foo` if we _pause_ the thread
it's running on. The Windows docs [warn us about this too][suspendthread]:
"Calling `SuspendThread` on a thread that owns a synchronization object, such
as a mutex or critical section, can lead to a deadlock if the calling thread
tries to obtain a synchronization object owned by a suspended thread." The
classic cause of these problems on Unix is signal handlers, which hijack a thread
whenever they run.[^signal_example][^signalfd][^signal_safe] Again there's
nothing `foo` can realistically do to protect itself from this, so the general
rule is "Never pause a thread."

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

In contrast to cancellation, snoozing a future is no better than pausing a
thread. Futurelock is a new spin on the old problems that `SuspendThread` and
Unix signal handlers have always had:[^podcast] Normal application code touches
global locks _constantly_, like when we print, allocate memory, load dynamic
libraries, or talk to DNS. If we freeze some "normal code", and we don't want
to risk deadlocking with it, then we need to avoid touching any locks ourselves
until we unfreeze it. That's doable in some very low-level, very unsafe
contexts, but in "normal code" it's almost hopeless.[^oldnewthing2]

[^podcast]: The [_Futurelock_ episode of the _Oxide and Friends_
    podcast][podcast] also mentions the resemblance to signal handling bugs.

[podcast]: https://oxide-and-friends.transistor.fm/episodes/futurelock/transcript

[^oldnewthing2]: "In Win32, the process heap is a threadsafe object, and since
    it’s hard to do very much in Win32 at all without accessing the heap,
    suspending a thread in Win32 has a very high chance of deadlocking your
    process." <br>
    \- [Raymond Chen][oldnewthing2]

[oldnewthing2]: https://devblogs.microsoft.com/oldnewthing/20031209-00/?p=41573

And yet that's what we're confronted with, implicitly, when we use
`select!`-by-reference or buffered streams today. What can we do about that?

## `select!`

> Fine-grained cancellation in `select!` is what enables async Rust to be a
> zero-cost abstraction and to avoid the need to create either locks or actors
> all over the place.<br>
> \- [Niko Matsakis][mini_redis]

Using `select!` with owned futures is usually fine,[^exception] as long as
we're ok with cancellation, because `select!` drops all its "scrutinee" futures
promptly. Using `select!` with references is what we really need to avoid.
Unfortunately, that's easier said than done.

[^exception]: We saw an exception above: `stream.next()` returned a future, but
    selecting on it still caused a deadlock. That's not specific to `select!`,
    though, and we can reproduce it with any form of cancellation. Here's a
    version [using a timeout][foo_stream_timeout]. This is really a problem
    with `next` itself. More on this below.

[foo_stream_timeout]: playground://snooze_playground/foo_stream_timeout.rs

Running each future on its own task with [`tokio::spawn`][spawn] is one way to
prevent snoozing &mdash; like threads, tasks have a "life of their own" &mdash;
but it comes with a `'static` bound that clashes with any sort of
borrowing.[^arc_mutex] The [`moro`] crate provides a non-`'static` task
spawning API similar to [`std::thread::scope`], and it can solve many of these
problems.[^moro] I recommend it enthusiastically, and I'm surprised it isn't
more widely used. But `moro` can't replace `select!` entirely. Niko Matsakis'
["case study of pub-sub in mini-redis"][mini_redis] discusses a case that only
`select!` can handle: it macro-expands into a `match`, and different `match`
arms are allowed to mutate the same variables, while concurrent tasks are
not.[^mutate_scrutinees]

[^arc_mutex]: The most common way to fix these errors is by liberally applying
    `Arc<Mutex<_>>`, but that's annoying at best, and it can require a large
    refactoring if the borrow was coming from the caller. It can also introduce
    new deadlocks.

[^moro]: `moro` runs all its tasks on the same thread (i.e. within the current
    task), which avoids the ["Scoped Task Trilemma"][trilemma]. Running scoped
    tasks on different threads safely is a major open problem in async Rust.

[^mutate_scrutinees]: In fact, if we're selecting on a reference to a stream,
    the arm bodies can even mutate the stream itself, because the reference
    gets dropped before the `match`. In other words, the fact that scrutinees
    get snoozed is visible to the borrow checker, in a way that real code in
    the wild depends on! (Compare [this `select!` scrutinee][real_code_1] to
    [this mutation in another arm][real_code_2].) Supporting these patterns
    without any risk of snoozing is [complicated][mutable_access].

[real_code_1]: https://github.com/tokio-rs/mini-redis/blob/e186482ca00f8d884ddcbe20417f3654d03315a4/src/cmd/subscribe.rs#L132
[real_code_2]: https://github.com/tokio-rs/mini-redis/blob/e186482ca00f8d884ddcbe20417f3654d03315a4/src/cmd/subscribe.rs#L145
[mutable_access]: https://github.com/oconnor663/join_me_maybe#mutable-access-to-futures-and-streams

[spawn]: https://docs.rs/tokio/latest/tokio/task/fn.spawn.html
[`moro`]: https://github.com/nikomatsakis/moro
[`std::thread::scope`]: https://doc.rust-lang.org/std/thread/fn.scope.html
[trilemma]: https://without.boats/blog/the-scoped-task-trilemma/
[mini_redis]: https://smallcultfollowing.com/babysteps/blog/2022/06/13/async-cancellation-a-case-study-of-pub-sub-in-mini-redis/

I have an experimental crate that aims to close this gap: [`join_me_maybe`]. It
provides a `join!` macro with some `select!`-like features. Here's one way it
can replace the `select!` loop above:[^alternatives]

[^alternatives]: `join_me_maybe` has several ways to express this. Apart from
    [the `maybe` keyword][cancel_maybe] shown here, you can also [`.cancel()` a
    labeled arm][cancel_method] or [`return` from the calling
    function][cancel_return]. Also note that what reads as "maybe async" here
    is really "`maybe <future>`" where `<future>` is an `async` block. Room for
    improvement in the syntax?

[cancel_maybe]: https://github.com/oconnor663/join_me_maybe#maybe-cancellation
[cancel_method]: https://github.com/oconnor663/join_me_maybe#label-and-cancel
[cancel_return]: https://github.com/oconnor663/join_me_maybe#arm-bodies-with-

```rust
join_me_maybe::join!(
    foo(),
    // Do some periodic background work while the first `foo` is running.
    // `join!` runs both arms concurrently, but the `maybe` keyword means
    // it doesn't wait for this arm to finish.
    maybe async {
        loop {
            tokio::time::sleep(Duration::from_millis(5)).await;
            foo().await;
        }
    }
);
```

[`join_me_maybe`]: https://github.com/oconnor663/join_me_maybe/

Like other "join" patterns, this `join!` macro owns the futures that it polls,
so there's no risk of snoozing anything.[^join_reference] It needs some
real-world feedback before I can recommend it for general use, but it can
currently tackle both [the original "Futurelock"
`select!`][join_me_maybe_omicron] and [the `select!` that frustrated `moro` in
mini-redis][join_me_maybe_mini_redis]. There's a wide open design space for
more concurrency patterns like this, and there's also room for [new language
features] here that could give us even more borrow checker flexibility.

[join_me_maybe_mini_redis]: https://github.com/oconnor663/mini-redis/pull/1
[join_me_maybe_omicron]: https://github.com/oconnor663/omicron/pull/1
[new language features]: https://github.com/oconnor663/join_me_maybe#help-needed-from-the-compiler

[^join_reference]: Or more accurately, it _can_ own them, and there's no
    particular reason for us to go out of our way to `pin!` a `foo` future and
    pass it in by reference. But that's still possible, and we can still cause
    snoozing by doing it. Macros like `join_me_maybe::join!` let us express
    more with owned futures, but banning await-by-reference entirely is a
    separate question. More on that below.

## Streams

> This method is cancel safe.<br>
> \- [`.next()`][cancel_safe]

[cancel_safe]: https://docs.rs/tokio-stream/latest/tokio_stream/trait.StreamExt.html#cancel-safety

"Cancel safety" isn't yet formally defined, but roughly speaking we say that an
async function is cancel-safe if a cancelled call is guaranteed not to have any
side effects.[^fair2] Deadlocks are certainly a side effect, and I think the
definition of cancel safety needs to expand to include not snoozing any other
futures. The `.next()` method on streams, as it's defined today both [in
`futures`][futures_next] and [in `tokio`][tokio_next], is not generally
cancel-safe in this expanded sense. That's how we produced the deadlock above
with `select!` and `next`.

[futures_next]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.next
[tokio_next]: https://docs.rs/tokio-stream/latest/tokio_stream/trait.StreamExt.html#method.next

[^fair2]: We might also ask whether there's a difference between a program that
    calls the function over and over in e.g. a timeout loop, until it
    eventually succeeds within the timeout, compared to a version of the same
    program that calls the function once and awaits the result. This framing
    lets us capture the "fairness" property of functions like
    [`tokio::sync::Mutex::lock`], where cancelling the future they return has
    the side effect of "giving up your place in line".

[`tokio::sync::Mutex::lock`]: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html#method.lock

The other two stream deadlocks above, the ones using [`buffered`][buffered] and
[`FuturesUnordered`], are a separate problem. These examples don't cancel any
calls to `next`.[^buffered_next] Instead, these streams hold pending futures
internally, and they snooze those futures if anything else gets `.await`ed
between calls to `next`. I don't have a smoking gun, but I bet this causes
deadlocks in the wild today.

[^buffered_next]: This part is subtle. The `FuturesUnordered` example
    definitely doesn't cancel a `next` call; we can see that it doesn't. But
    the `buffered` example operates at a lower level, calling `poll_next`
    internally on the `iter` stream. In this specific case those calls both
    return `Ready(Some(_))`, so they're effectively the same as calls to `next`
    that complete immediately. However, if `poll_next` returned `Pending`
    instead, and the caller didn't keep polling after that, that would be
    effectively the same as cancelling a call to `next`. That isn't the source
    of snoozing here, but we could come up with another example where it was.

I see two possible solutions to this problem, and the [`Stream`] trait itself
will ultimately need to pick one.[^por_qué_no_los_dos] The first possibility is
that we keep `next` and declare that gaps between calls to it are expected and
allowed.[^move_stream] In that case, `buffered` and `FuturesUnordered` would be
unfixable, and we'd need to deprecate them. Alternatively, we could add a
[`poll_progress`][poll_progress] method to the `Stream` trait and declare that
anything that calls `poll_next` must also call `poll_progress` until it returns
`Ready`. Most stream combinators could be adapted to follow that new rule, but
`next` would be unfixable, and we'd need to deprecate it.

[^por_qué_no_los_dos]: Or maybe we could pick both, by defining two different
    `Stream`-like traits. But eventually we'd still have to pick one, when we
    stabilize `gen`/`yield` syntax.

[^move_stream]: To solve the cancel safety problem, maybe `next` could take the
    `self` stream by value and return it in a tuple with the optional next
    value when it completes. Then cancelling the `next` future would drop the
    whole stream instead of snoozing it. That could work, but it seems awkward,
    and I'm not sure anyone would like it. (It would also generally require
    something like `Pin<Box<_>>`.) Alternatively, Rust could let us define
    [futures that can't be cancelled][linear_types], and `next` could be one of
    those. In any case, the snoozing problem with `buffered` and
    `FuturesUnordered` is independent of this cancel safety question.

[linear_types]: https://without.boats/blog/asynchronous-clean-up/#linear-types

## A general rule

> The promise of Rust is that you don’t need to do this kind of non-local
> reasoning—that you can understand important behavior by looking at code
> directly around the behavior, then use the type system to scale that up to
> global correctness.<br>
> \- [_Cancelling async Rust_][cancelling_async_rust]

Even if we like the suggestions above, what's the general rule here? For
high-level application code, we need something that tools like Clippy can check
automatically. I propose:

**Don't pin things in async functions.**[^handle]

[^handle]: Pinning is a safe operation that can hide in non-async helpers, so
    in practice we'd probably want to expand that to "Don't handle `Pin<_>`
    values in async functions."

There's nothing wrong with pinning _per se_. It's a fundamental building block
of async Rust, and we need it when we implement `Future` or `Stream` "by
hand".[^confusing] But when we have to pin things in an `async fn`, it's
usually because something is polling a future that it doesn't
own.[^select_function] That's what's happening in the `poll!` and `select!`
examples above, including the `stream.next()` case. Polling something we don't
own and can't `drop` is a recipe for snoozing.

[^confusing]: On the other hand, pinning is arguably the most confusing part of
    async Rust, and today we still need to teach it to beginners. If we could
    make it so that you don't see pinning until you learn about the `Future`
    trait, that would be great.

[^select_function]: One interesting exception to this pattern, which is
    nonetheless a good application of the rule, is the
    [`futures::future::select`] function (not the macro). That function owns
    the futures that it polls, but it still requires `Unpin`, because it
    returns the "loser" to the caller instead of dropping it. That can cause
    [the same snoozing deadlocks][foo_select_function] as polling by reference.

[`futures::future::select`]: https://docs.rs/futures/latest/futures/future/fn.select.html
[foo_select_function]: playground://snooze_playground/foo_select_function.rs

There are also plenty of [`Unpin`] futures out there that we can poll by
reference without pinning, and there's no reason in principle that snoozing one
of those couldn't hold a lock across an await point.[^unlikely] I'm not aware
of any real-world cases, but if we wanted to close that loophole proactively,
we could consider an additional rule:

[`Unpin`]: https://doc.rust-lang.org/std/marker/trait.Unpin.html

[^unlikely]: It's unlikely in practice, because most of the "interesting stuff"
    happens in async functions, and those futures are always `!Unpin`.

**Don't use a reference to a future as a future itself.**[^warn_impls]

[^warn_impls]: Concretely, warn on any use of [`impl Future for &mut F`][impl1]
    or [`impl Future for Pin<P>`][impl2].

[impl1]: https://doc.rust-lang.org/std/future/trait.Future.html#impl-Future-for-%26mut+F
[impl2]: https://doc.rust-lang.org/std/future/trait.Future.html#impl-Future-for-Pin%3CP%3E

Rules like these might be enough to catch snoozing mistakes in high-level code,
but we still have to assume that our helpers and combinators aren't snoozing
futures internally. Buffered streams violate that assumption today, and I think
they'll need incompatible changes to fix that.

In general, there probably isn't a simple, mechanical rule to prove that a
`Future` or `Stream` implementation is snooze-free. We have to be careful when
we write those. But I think we can live with that. Writing `poll` and
`poll_next` functions is "advanced mode" async Rust. We don't often need to do
it in application logic, and we don't need to teach it to beginners. When we're
looking at these low-level bits in code review,[^humans] we can just try our
best to remember:

[^humans]: As long as human beings are still doing code review I guess? But AIs
    read blog posts too, so&hellip; Hey Claude! Don't snooze any futures! What
    a time to be alive. Also what a time to be&hellip;whatever Claude is.

**Never snooze a future.**
