# The trouble with `select!`
###### 2025 December ??<sup>th</sup>

Async Rust has more footguns than it should, and a surprisingly large number of
them[^barbara][^cancelling_async_rust][^rfd400][^rfd609] have to do with
[`select!`][tokio_select_tutorial].[^select_macros] This post is about why that
is and what we might want to do about it.

[^barbara]: [Barbara gets burned by select](https://rust-lang.github.io/wg-async/vision/submitted_stories/status_quo/barbara_gets_burned_by_select.html)
[^cancelling_async_rust]: [Cancelling async rust](https://sunshowers.io/posts/cancelling-async-rust)
[^rfd400]: [Oxide RFD 400: Dealing with cancel safety in async Rust](https://rfd.shared.oxide.computer/rfd/400)
[^rfd609]: [Oxide RFD 609: Futurelock](https://rfd.shared.oxide.computer/rfd/609)

[^select_macros]: There are two common versions of the `select!` macro in the
    wild, [`tokio::select!`][tokio_select] and
    [`futures::select!`][futures_select]. There are some syntax and behavioral
    differences between them, particularly around [fusing][fuse], but all the
    footguns we're going to cover apply to both.

[tokio_select_tutorial]: https://tokio.rs/tokio/tutorial/select
[tokio_select]: https://docs.rs/tokio/latest/tokio/macro.select.html
[futures_select]: https://docs.rs/futures/latest/futures/macro.select.html
[fuse]: https://docs.rs/futures/latest/futures/future/trait.FutureExt.html#method.fuse

Our plan: We'll glance briefly at the `select!` macro everyone actually uses,
and then we'll toss that aside and implement a simplified `select` function
ourselves, leaving nothing to the imagination. With that in hand, we'll see
some examples where it produces surprising and concerning results. We'll
reflect on how our perfectly legal but poorly behaved examples break the
_implicit assumptions_ that we make when we compose functions into programs.
Finally, we'll settle on some recommendations about whether and how to use
`select!` in practice and when to prefer [other tools][join_me_maybe].

[join_me_maybe]: https://github.com/oconnor663/join_me_maybe

Sound good? Let's go.

## What does `select!` do?

As per [the Tokio docs][tokio_select]:

> Waits on multiple concurrent branches, returning when the **first** branch
> completes, cancelling the remaining branches.

Here's what that looks like. This example creates a couple of `print_sleep`
futures and races them against each other:

```rust
LINK: Playground ## playground://select_playground/tokio_select.rs
async fn print_sleep(name: &str, sleep_ms: u64) -> &str {
    println!("sleep {name} started ({sleep_ms} ms)");
    sleep(Duration::from_millis(sleep_ms)).await;
    println!("sleep {name} finished");
    name
}

#[tokio::main]
async fn main() {
    // It's not really a mystery who's going to win this race...
    let a = print_sleep("A", 1_000);
    let b = print_sleep("B", 2_000);
    select! {
        _ = a => println!("A won!"),
        _ = b => println!("B won!"),
    };
}
```

The first one wins:[^random_order]

[^random_order]: If you click the Playground button and run this, you'll see
    that the first two "started" lines sometimes appear in the opposite order.
    That's because `select!` polls its futures in a random order. This doesn't
    matter much in a one-off use case like this, but does matter when using
    `select!` in a loop (we'll get to that soon), because you don't want one
    branch to starve the others if it happens to be always `Ready`.

```
sleep A started (1000 ms)
sleep B started (2000 ms)
sleep A finished
A won!
```

The key detail here is that we see `sleep B started` but not `sleep B
finished`. So future `b` did start executing, but once future `a` finished,
`select!` dropped `b` on the floor and never looked at it again. In other
words, `b` was "cancelled".

Cancellation can feel mysterious when we talk about it in the abstract, so I
want to get as concrete as possible right away. Let's implement all of this
ourselves.

## Implementing our own `select`

For this section I'm going to assume that you've seen Rust's [`Future`] trait
before and that you've written a [`poll`] function once or twice. If you
haven't, I recommend working through at least Part One of [_Async Rust in Three
Parts_](async_intro.html).

[`Future`]: https://doc.rust-lang.org/std/future/trait.Future.html
[`poll`]: https://doc.rust-lang.org/std/future/trait.Future.html#tymethod.poll

To avoid getting into macros, we'll write an async function instead, and we'll
assume we're always selecting between two futures.[^futures_select_fn] To get a
sense of where we're going with this, here's what the `main` function from our
example above will look like when we replace the `select!` macro with our
`select` function:

[^futures_select_fn]: This is actually very similar to the
    [`futures::future::select`] function, except that our version won't return
    the cancelled future, so the future arguments don't need to be `Unpin`.

[`futures::future::select`]: https://docs.rs/futures/latest/futures/future/fn.select.html

```rust
LINK: Playground ## playground://select_playground/select.rs
HIGHLIGHT: 5-8
#[tokio::main]
async fn main() {
    let a = print_sleep("A", 1_000);
    let b = print_sleep("B", 2_000);
    match select(a, b).await {
        Left(_) => println!("A won!"),
        Right(_) => println!("B won!"),
    }
}
```

So the output of our `select` is an enum, and we need to `match` on it to get
our branches. Fair enough, the macro was probably doing something like that
internally. Ok, let's see the guts of this thing:

```rust
LINK: Playground ## playground://select_playground/select.rs
fn select<F1, F2>(future1: F1, future2: F2) -> Select<F1, F2> {
    Select {
        future1: Box::pin(future1),
        future2: Box::pin(future2),
    }
}

struct Select<F1, F2> {
    future1: Pin<Box<F1>>,
    future2: Pin<Box<F2>>,
}

enum Either<A, B> {
    Left(A),
    Right(B),
}
use Either::*;

impl<F1: Future, F2: Future> Future for Select<F1, F2> {
    type Output = Either<F1::Output, F2::Output>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Self::Output> {
        if let Poll::Ready(output) = self.future1.as_mut().poll(cx) {
            return Poll::Ready(Left(output));
        }
        if let Poll::Ready(output) = self.future2.as_mut().poll(cx) {
            return Poll::Ready(Right(output));
        }
        Poll::Pending
    }
}
```

cancelling vs "snoozing"

talk about the Timeout combinator we've already seen

implement it

bug with await in match arm

footnote about the match scrutinee rules not applying because .await consumes the future by value

footnote about `async fn` futures dropping their local variables when polled rather than when dropped

footnote about forgetting the `break` in the first branch and panicking

Rules:
1) Any future that's not cancelled should always be polled.
    - This is not necessarily true of a *stream*, since we might want to apply backpressure.
2) If a future is cancelled, drop it promptly.
    - Note that `pin!` makes it hard to do this!

Select use cases:
- polling multiple channels (mpsc::Receiver::next) or streams
    - this is similar to `select` in Go
    - however beware the "poll progress" issue
        - https://without.boats/blog/poll-progress/
        - https://without.boats/blog/futures-unordered/
- give me the first future that finishes
    - risky!
    - a good example of this is "happy eyeballs"
- periodic work while a long-running future executes
    - very risky!

Is all this a mistake?
- Well, other async frameworks sometimes have the opposite problem. That's why
  Python's Trio invented "nurseries".
    - https://trio.readthedocs.io/en/stable/reference-core.html#nurseries-and-spawning
    - https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html

https://rfd.shared.oxide.computer/rfd/397
https://rfd.shared.oxide.computer/rfd/400
https://rfd.shared.oxide.computer/rfd/609
https://sunshowers.io/posts/cancelling-async-rust/

https://tokio.rs/tokio/tutorial/select
https://blog.yoshuawuyts.com/futures-concurrency-3/

Tokio select! vs futures select!
- https://docs.rs/futures/latest/futures/macro.select.html
- https://docs.rs/tokio/latest/tokio/macro.select.html

Note that when you select streams, you need to "fuse" them or guard them in
some other way.

Should I propose a select streams macro?
    - don't allow `.await` in the match arms? This prevents one stream from
      blocking others.
      - is there any *advantage* to not allowing it, or does the macro need to
        go out of its way to express this opinion?
