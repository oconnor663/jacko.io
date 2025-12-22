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
hangs and deadlocks in async Rust, including the ["Futurelock"][futurelock]
case study from the folks at Oxide. I'm going to argue that snoozing is
_always_ a bug, and that the async patterns that expose us to it can and should
be replaced.

[^three_parts]: If you aren't familiar with the [`poll`] and [`Waker`]
    machinery that makes async Rust tick, I recommend reading at least part one
    of [Async Rust in Three Parts][three_parts].

[`poll`]: https://doc.rust-lang.org/std/future/trait.Future.html#tymethod.poll
[`Waker`]: https://doc.rust-lang.org/std/task/struct.Waker.html
[three_parts]: async_intro.html

[^starvation]: Snoozing is similar to "starvation", but starvation usually
    means a call to `poll` is blocking instead of returning quickly, and the
    executor can't poll anything else while it waits. Snoozing is when the
    executor is running fine, but some futures still aren't getting polled.

Before we dive in, I want to be clear that snoozing and cancellation aren't the
same thing, though they do overlap. If a snoozed future eventually wakes up,
then clearly it wasn't cancelled. But a cancelled future can also be snoozed,
if there's a gap between when it's last polled and when it's finally
dropped.[^define_cancellation] Cancellation bugs are a [big topic] in async Rust,
and it's good that we're talking about them, but cancellation _itself_ isn't a bug. Snoozing on
the other hand is _always_ a bug, and I don't think we talk about it enough.

[^define_cancellation]: We often say that cancelling a future _means_ dropping
    it, but a future that's never going to be polled again has also arguably
    been cancelled, even if it hasn't yet been dropped. Which definition is
    right? If we can agree that snoozing is always a bug, then we don't have to
    worry too much about this question, because we'll always drop cancelled
    futures promptly.
    <br>
    &emsp;
    I think asking "when is a future really cancelled?" is like asking "when is
    an object really dropped?" Is it when `drop` is called, or after `drop`
    returns? A compiler or a debugger might need to have an opinion about this,
    but application code doesn't, because application code doesn't see objects
    that are halfway through `drop`. Similarly, we shouldn't let application
    code see a future that's cancelled but not dropped.

[big topic]: https://sunshowers.io/posts/cancelling-async-rust/

## Deadlocks

Snoozing usually leads to mysterious latency spikes or timeouts, but the
clearest and most dramatic examples are deadlocks. The folks at Oxide called
this "futurelock" in [the case study][futurelock] they published in
October.[^chilling] I think the best way to understand the problem is to shrink
their example down further and look at several variations of it.

[^chilling]: To me the most chilling line from that post is "There’s no one
    abstraction, construct, or programming pattern we can point to here and say
    'never do this'." My position here is that there are constructs and
    patterns that we can point to, and that there are viable alternatives that
    we can recommend.

Our subject in all these examples will be the function `foo`, which takes an
async lock and does some fake work:[^nothing_wrong]

[^nothing_wrong]: There's [an important section][what_kind_of_mutex] in the
    Tokio docs that recommends using ordinary mutexes instead of async ones
    most of the time. That's good advice, but I want to emphasize that there's
    nothing _wrong_ with `foo`. We could make examples like these out of any
    form of synchronization: locks, semaphors, channels, even [`OnceCell`]s.

[what_kind_of_mutex]: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html#which-kind-of-mutex-should-you-use
[`OnceCell`]: https://docs.rs/tokio/latest/tokio/sync/struct.OnceCell.html

```rust
LINK: Playground ## playground://snooze_playground/foo.rs
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

static LOCK: Mutex<()> = Mutex::const_new(());

async fn foo() {
    let _guard = LOCK.lock().await;
    sleep(Duration::from_millis(1)).await;
}
```

We can call `foo` as many times as we like without causing trouble:

```rust
LINK: Playground ## playground://snooze_playground/foo.rs
foo().await;
foo().await;
foo().await; // ok
```

And we can call `foo` concurrently, using `join!`:

```rust
LINK: Playground ## playground://snooze_playground/foo_join.rs
join!(foo(), foo(), foo()); // ok
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
