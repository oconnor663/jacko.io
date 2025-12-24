# Never snooze a future
###### 2025 December ??<sup>th</sup>

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
that "snoozing".[^three_parts][^starvation] Snoozing is to blame for a lot of
hangs and deadlocks in async Rust, including the recent
["Futurelock"][futurelock] case study from the folks at Oxide.[^chilling] I'm
going to argue that snoozing is almost always a bug, that the tools and
patterns that expose us to it should be considered harmful, and that reliable
and convenient replacements are possible.

[^three_parts]: If you aren't familiar with the [`poll`] and [`Waker`]
    machinery that makes async Rust tick, I recommend reading at least part one
    of [Async Rust in Three Parts][three_parts].

[`poll`]: https://doc.rust-lang.org/std/future/trait.Future.html#tymethod.poll
[`Waker`]: https://doc.rust-lang.org/std/task/struct.Waker.html
[three_parts]: async_intro.html

[^starvation]: Snoozing is similar to "starvation", but starvation usually
    means that a call to `poll` is blocking instead of returning quickly, which
    stops the executor from polling anything else while it waits. Snoozing is
    when the executor is running fine, but some futures still aren't getting
    polled.

[^chilling]: To me the most chilling line from that post is "There’s no one
    abstraction, construct, or programming pattern we can point to here and say
    'never do this'." I partly disagree. I think we *can* point the finger
    squarely at patterns like `select!` and buffered streams. But if we're
    going to do that, we need to tell folks what they should use instead,
    especially in tricky cases involving cancellation or local borrowing. In
    the second half of this post, I'll make some suggestions.

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
    sleep(Duration::from_millis(1)).await; // very important work
}
```

As we go along, imagine that `foo` is buried three crates deep in some
dependency we've never heard of. In real life the lock, the future that's
holding it, and the mistake that snoozes that future might all be far apart
from each other.[^ghidra] With that in mind, here's the minimal futurelock:

[^ghidra]: In the [original issue thread][gh9259] that inspired "Futurelock",
    they resorted to booting up [Ghidra] and staring at core dumps. Truly the
    stuff of nightmares.

[gh9259]: https://github.com/oxidecomputer/omicron/issues/9259
[Ghidra]: https://github.com/NationalSecurityAgency/ghidra

```rust
LINK: Playground ## playground://snooze_playground/foo_poll.rs
let future = pin!(foo());
_ = poll!(future);
foo().await; // Deadlock!
```

[`poll!`](https://docs.rs/futures/latest/futures/macro.poll.html) is a macro
that you rarely see outside of examples.[^struct] It polls the first `future`
exactly once, which brings it to the point where it's acquired the `LOCK` and
started sleeping. Then we call `foo` again, it blocks on the same `LOCK`, and
because we're only `.await`ing the second call to `foo`, we're deadlocked.

[^struct]: If you don't trust the macro, you can do the same thing with [an
    ordinary struct that implements `Future`][poll_struct].

[poll_struct]: playground://snooze_playground/foo_poll_struct.rs

You probably won't see anyone using `poll!` like that in the wild, but here's
effectively the same thing using
[`select!`](https://docs.rs/tokio/latest/tokio/macro.select.html):[^arms]

[^arms]: It works equally well to put calls to `foo` [in the `select!` arm
    bodies][foo_select_arms].

[foo_select_arms]: playground://snooze_playground/foo_select_arms.rs

```rust
LINK: Playground ## playground://snooze_playground/foo_select.rs
let future1 = pin!(foo());
let future2 = pin!(foo());
select! {
    _ = future1 => {}
    _ = future2 => {}
}
foo().await; // Deadlock!
```

In this case `select!` polls both futures. Whichever one goes first[^random]
acquires the lock and sleeps, while the other waits on the lock and takes a
"place in line".[^fairness] After the sleep, `select!` finishes the first
future and returns.[^cancel] The following call to `foo` "gets in line" behind
the second future, and we're deadlocked again.

[^random]: `select!` chooses randomly.

[^fairness]: In other words, unlike `std::sync::Mutex`, [Tokio's
    `Mutex`][tokio_mutex] is "fair". If it wasn't, we'd need to make sure the
    second future got polled one more time and actually acquired the lock. We
    could make that work by adding another short sleep in `foo` after the
    critical section. To be clear, the fairness property itself is not "at
    fault" for this deadlock.

[tokio_mutex]: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html

[^cancel]: When one arm returns `Ready`, `select!` nominally cancels the
    others. But because of how the [`pin!`] macro works, we're actually
    selecting on these future by reference, and dropping a reference has no
    effect.

[`pin!`]: https://doc.rust-lang.org/std/pin/macro.pin.html

We can also do same thing by using `select!` with [streams]:[^stream_snoozing]

[streams]: https://tokio.rs/tokio/tutorial/streams

[^stream_snoozing]: This is subtle. There's nothing wrong with calling
    `.next().await` once and then not again for a while. That's not snoozing;
    that's just normal iteration. The problem here is that we haven't paused
    the stream at a _yield point_, where it would be if `next` had just given
    us an item. (Note that there's no way for a stream to yield an item inside
    `foo`'s critical section, except by committing a snoozing crime internally.
    That's not the case here, but see `FuturesUnordered` below). These streams
    only snooze `foo` because we snooze them. When we do that, they snooze at
    an _await point_, i.e. some random line of async code that might not even
    know it's in a stream. If we start a call to `next` (in general, if
    [`poll_next`] returns `Pending`), then we either need to keep driving the
    stream until it yields something or else drop the _whole stream_. (TODO:
    There should probably be an exception for `Unpin` streams like channels.)

[`poll_next`]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html#tymethod.poll_next

```rust
LINK: Playground ## playground://snooze_playground/foo_select_streams.rs
let mut stream1 = pin!(stream::once(foo()));
let mut stream2 = pin!(stream::once(foo()));
select! {
    _ = stream1.next() => {}
    _ = stream2.next() => {}
}
foo().await; // Deadlock!
```

Speaking of streams, a related class of deadlocks that you might see in the
wild is caused by ["buffered"] streams:

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
closure is running, the other `foo` in the buffer is snoozed.

Buffered streams are a wrapper around either [`FuturesOrdered`] or
[`FuturesUnordered`], and we can trigger the same bug by looping over either of
those directly:[^stream_fault]

[`FuturesOrdered`]: https://docs.rs/futures/latest/futures/stream/struct.FuturesOrdered.html
[`FuturesUnordered`]: https://docs.rs/futures/latest/futures/stream/struct.FuturesUnordered.html

[^stream_fault]: Compare this example to the `stream::once` example above.
    There we were "at fault" for snoozing the stream. But here, our program is
    doing its best to drive each `StreamExt::next` call to completion, and the
    real `foo` snoozer is `FuturesUnordered` itself!

```rust
LINK: Playground ## playground://snooze_playground/foo_unordered.rs
let mut futures = FuturesUnordered::new();
futures.push(foo());
futures.push(foo());
while let Some(_) = futures.next().await {
    foo().await; // Deadlock!
}
```

These patterns are common in async Rust, and many of them are ticking time
bombs, sitting around until some unlikely timeout fires, some channel fills up,
or some dependency adds an internal lock. This is a problem.

But regular non-async Rust doesn't have this sort of problem. Why not?

## Threads

> How many times does<br>
> it have to be said: Never<br>
> call `TerminateThread`.<br>
> \- [Larry Osterman][terminate_thread]

[terminate_thread]: https://devblogs.microsoft.com/oldnewthing/20150814-00/?p=91811

## Deprecated Idioms

## Alternatives


https://smallcultfollowing.com/babysteps/blog/2022/06/13/async-cancellation-a-case-study-of-pub-sub-in-mini-redis/

async generators vs futures:

It's true that a generator holding lock across a yield point can cause the same
deadlocks as a snoozed future. But a key difference is that a future can
interact with locks that are buried in layers of function calls and never
appear anywhere near the bug. Whereas to hold a lock across a generator yield
the guard has to actually be *returned* to you. It could still be abstracted
inside some structs, but in general "does holding this object hold a lock" is
much more visible than "does calling this function touch a lock". In other
words, the critical section of the lock has to actually overlap with the body
of the generator.

HOWEVER, you still need to worry about snoozing a generator at some .await that
isn't a yield point. That's really no different from snoozing a future.

---

Note that the example in the Futurelock blog post can be fixed by dropping the
cancelled future promptly, but the original but that motivated the whole
article can't be fixed that way.
