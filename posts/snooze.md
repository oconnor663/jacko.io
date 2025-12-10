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
> \- [_FuturesUnordered and the order of futures_][order]

[order]: https://without.boats/blog/futures-unordered/

Async Rust has a category of footguns that has lasted longer than it should, in
part because we haven't given it a name. I call it "snoozing".[^starvation]
Snoozing is when we don't poll (or drop) a future promptly after it invokes its
`Waker`.[^three_parts] I'm going to make the case that snoozing is _always_ a
bug. From there, I'll argue that some widely used async idioms[^idioms] should
be avoided or deprecated, because they're too prone to snoozing. Finally, I'll
suggest some alternatives that don't have this particular footgun.

[^starvation]: Snoozing is loosely similar to "starvation", but starvation
    usually refers to when polling takes too long, either because of
    (accidental) synchronous IO or some long-running computation. That blocks
    an entire task plus an executor thread, or a whole single-threaded
    executor. Snoozing is when one future isn't getting polled, even though
    it's ready to make progress, and the task that owns it is running smoothly.

[^three_parts]: If you aren't familiar with the [`poll`] and [`Waker`]
    machinery that makes async Rust tick, I recommend reading at least part one
    of [Async Rust in Three Parts][three_parts].

[`poll`]: https://doc.rust-lang.org/std/future/trait.Future.html#tymethod.poll
[`Waker`]: https://doc.rust-lang.org/std/task/struct.Waker.html
[three_parts]: async_intro.html

[^idioms]: In particular, awaiting-by-reference (which in practice usually
    means `select!`-by-reference) and also [buffered streams][buffered].

[buffered]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.buffered

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
