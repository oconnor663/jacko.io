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
going to argue that snoozing is _always_ a bug, and that the async patterns
that expose us to it can and should be replaced.

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
    squarely at patterns like `select!` and buffered streams. But when we do
    that, we need to have better options ready to go, especially for tricky
    cases involving cancellation or local borrowing. In the second half of this
    post, I'll make some suggestions.

Before we dive in, I want to be clear that snoozing and cancellation are
different things. If a snoozed future eventually wakes up, then clearly it
wasn't cancelled. On the other hand, a cancelled future can also be snoozed, if
there's a gap between when it's last polled and when it's finally
dropped.[^define_cancellation] Cancellation bugs are a [big topic] in async
Rust, and it's good that we're talking about them, but cancellation _itself_
isn't a bug. Snoozing is _always_ a bug, and I don't think we talk about it
enough.

[^define_cancellation]: We often say that cancelling a future _means_ dropping
    it, but a future that's never going to be polled again has also arguably
    been cancelled, even if it hasn't yet been dropped. Which definition is
    better? I'm not sure, but if snoozing is always a bug, then the difference
    only matters to buggy programs.

[big topic]: https://sunshowers.io/posts/cancelling-async-rust/

## Deadlocks

Snoozing can cause mysterious latency spikes or timeouts, but the clearest and
most dramatic snoozing bugs are deadlocks ("futurelocks"). Let's look at
several tiny examples. Our test subject today is `foo`, a toy function that
takes a private async lock and pretends to do some work:

```rust
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

static LOCK: Mutex<()> = Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    sleep(Duration::from_millis(1)).await; // very important work
}
```

Keep in mind that `foo` could be buried three crates deep in some dependency
you've never heard of. None of the following examples need us to call it
directly.[^nothing_wrong] With that in mind, here's the minimal possible
futurelock:

[^nothing_wrong]: I also want to emphasize that there's nothing "wrong" with
    `foo`. We could make examples like these with of any form of async
    blocking: semaphors, bounded channels, even [`OnceCell`]s. There's some
    [interesting advice in the Tokio docs][what_kind_of_mutex] about using
    `std::sync::Mutex` instead of `tokio::sync::Mutex` as much as possible, and
    that's good advice, but consider that even `tokio::sync::mpsc` channels
    [use a semaphor internally][internally].

[what_kind_of_mutex]: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html#which-kind-of-mutex-should-you-use
[`OnceCell`]: https://docs.rs/tokio/latest/tokio/sync/struct.OnceCell.html
[internally]: https://github.com/tokio-rs/tokio/blob/0ec0a8546105b9f250f868b77e42c82809703aab/tokio/src/sync/mpsc/bounded.rs#L162

```rust
LINK: Playground ## playground://snooze_playground/foo_poll.rs
let mut future = pin!(foo());
_ = poll!(&mut future);
foo().await; // Deadlock!
```

[`poll!`](https://docs.rs/futures/latest/futures/macro.poll.html) is a macro
that you rarely see outside of examples.[^struct] It polls `future` exactly
once, which brings it to the point where it's acquired the `LOCK` and started
sleeping. Then we call `foo` again, it blocks on the same `LOCK`, and because
we're only `.await`ing the second call to `foo`, we're deadlocked.

[^struct]: If you don't trust the macro, we can do the same thing with [an
    ordinary struct that implements `Future`][poll_struct].

[poll_struct]: playground://snooze_playground/foo_poll_struct.rs

You probably won't see `poll!` much the wild, but here's effectively the same
bug using [`select!`](https://docs.rs/tokio/latest/tokio/macro.select.html):

```rust
LINK: Playground ## playground://snooze_playground/foo_select.rs
let mut future1 = pin!(foo());
let mut future2 = pin!(foo());
select! {
    _ = &mut future1 => {}
    _ = &mut future2 => {}
}
foo().await; // Deadlock!
```

In this case `select!` polls both futures. Whichever one goes first[^random]
acquires the lock and goes to sleep, while the other waits on the lock and
receives a "place in line".[^fairness] After the sleep, `select!` finishes the
first future and returns.[^cancel] The following call to `foo` "gets in line"
behind the second future, and we're deadlocked again.

[^random]: `select!` chooses randomly.

[^fairness]: In other words, unlike `std::sync::Mutex`, Tokio's `Mutex` is
    "fair". If it wasn't, then to reproduce this deadlock we'd need to make
    sure the second future got polled one more time and actually acquired the
    lock. We could make that work by adding another short sleep in `foo` after
    the critical section. To be clear, the fairness property itself is
    certainly not "at fault" for this deadlock.

[^cancel]: When one arm returns `Ready`, `select!` nominally cancels all the
    others. But we're selecting on these future by reference, and dropping a
    reference has no effect.

It's starting to look like `&mut future` might be the source of our troubles
here, but it turns out we can do the exact same thing using [`StreamExt::next`]
instead:[^stream_snoozing]

[`StreamExt::next`]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.next

[^stream_snoozing]: This is subtle. It's not a problem to call `Stream::next`
    and then not call it again for a while. That's just normal iteration. The
    problem is that we cancelled a call to `StreamExt::next`. We haven't paused
    the stream at a _yield_ point, but rather at an _await_ point. Note that
    there's no way for any stream to yield an item inside of `foo`'s critical
    section, other than by committing a snoozing crime of its own internally.
    These streams haven't committed any crimes. They'll only snooze `foo` if
    _we_ snooze _them_. In general, if you start calling `StreamExt::next` or
    `Stream::poll_next`, then you either need to keep driving it until the next
    item or else drop the _whole stream_. (There should probably be an
    exception for `Unpin` streams like channels. I need to think about it.)

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

Speaking of streams, another version of this bug that you'll see in the wild
comes from ["buffered"] streams:

["buffered"]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.buffered

```rust
LINK: Playground ## playground://snooze_playground/foo_buffered.rs
futures::stream::iter([foo(), foo()])
    .buffered(2)
    .for_each(|_| foo()) // Deadlock!
    .await;
```

Here the "buffer" starts polling both of the first two `foo` futures
concurrently. When the first one finishes, control passes to the `for_each`
closure. While that closure is running, the remaining buffered `foo` is
snoozed.

Buffered streams are a thin wrapper around either [`FuturesOrdered`] or
[`FuturesUnordered`], and we can do the same thing by looping over them
directly:[^stream_fault]

[`FuturesOrdered`]: https://docs.rs/futures/latest/futures/stream/struct.FuturesOrdered.html
[`FuturesUnordered`]: https://docs.rs/futures/latest/futures/stream/struct.FuturesUnordered.html

[^stream_fault]: We can compare this case to the `StreamExt::next` example
    above. There, the program was "at fault" for snoozing the stream. But here,
    the program is doing its best to drive each `StreamExt::next` call to
    completion, and the real `foo` snoozer is `FuturesUnordered` itself!

```rust
LINK: Playground ## playground://snooze_playground/foo_unordered.rs
let mut futures = FuturesUnordered::new();
futures.push(foo());
futures.push(foo());
while let Some(_) = futures.next().await {
    foo().await; // Deadlock!
}
```

## Threads and Fork

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
