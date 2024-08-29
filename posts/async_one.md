# Async Rust, Part One: Why?
###### \[date]

- Part One: Why? (you are here)
- [Part Two: How?](async_two.html)
- [Part Three: More!](async_three.html)

Async/await, or async IO, is a new-ish language feature that lets us do more
than one thing at a time. Rust has had async/await since 2019. It's especially
popular with websites and network services that handle many connections at
once, because running lots of async "futures" or "tasks" is more efficient than
running lots of threads.[^lots]

[^lots]: "Lots" here usually means 10k or more.

At a very high level, using threads means asking your OS and your hardware to
run different jobs in parallel for you, but using async/await means
reorganizing your own code to run those jobs yourself.[^concurrency] That's
partly why it's more efficient, but for the same reason the details of async
tend to "leak" into your code, and that makes it harder to learn. This series
is a details-first introduction to async Rust, focused on translating async
examples into ordinary Rust code that we can read and play with.

[^concurrency]: The famously confusing technical terms for this distinction are
    "parallelism" vs "concurrency". I don't think they're helpful for teaching.

The examples in this series will use lots of traits, generics, closures, and
threads. I'll assume that you've written some Rust before and that you've read
[The Rust Programming Language] or similar.[^ch_20] If not, this will be a bit
of a firehose, and you might need to refer back to something like [Rust By
Example] as you go.[^books]

[The Rust Programming Language]: https://doc.rust-lang.org/book/
[Rust By Example]: https://doc.rust-lang.org/rust-by-example/

[^ch_20]: The multithreaded web server project in [Chapter 20] is particularly
    relevant.

[Chapter 20]: https://doc.rust-lang.org/book/ch20-00-final-project-a-web-server.html

[^books]: If you're the sort of programmer who doesn't like learning new
    languages from books, consider [this advice from Bryan Cantrill][advice],
    who's just like you: "With Rust, you need to _learn_ it&hellip;buy the
    book, sit down, read the book in a quiet place&hellip;Rust rewards that."

[advice]: https://youtu.be/HgtRAbE1nBM?t=3913

Most of our async examples will use the [Tokio async
"runtime"][Tokio].[^more_than_one] You could say that the goal of this series
is to give us lots of practical intuition about what an async runtime is and
what it does.

[Tokio]: https://tokio.rs/

[^more_than_one]: There are several async runtimes available in Rust, but the
    differences between them aren't important for this series. Tokio is the
    most popular and the most widely supported, so it's a good default.

## Threads

Here's an example function `foo` that takes a second to run:

```rust
fn foo(n: u64) {
    println!("start {n}");
    std::thread::sleep(Duration::from_secs(1));
    println!("end {n}");
}
```

If we want to make several calls to `foo` at the same time, we can spawn a
thread for each one. Click on the Playground button to see that this takes one
second instead of ten:[^order]

[^order]: You'll probably also see the "start" and "end" prints appear out of
    order. One of the tricky things about threads is that we don't which one
    will finish first.

```rust
LINK: Playground playground://async_playground/threads.rs
let mut threads = Vec::new();
for n in 1..=10 {
    threads.push(std::thread::spawn(move || foo(n)));
}
for thread in threads {
    thread.join().unwrap();
}
```

We can bump that up to [a hundred threads][hundred_threads], and it works just
fine. But if we try to run [a thousand
threads][thousand_threads],[^thread_limit] it doesn't work anymore:

[hundred_threads]: playground://async_playground/threads_100.rs
[thousand_threads]: playground://async_playground/threads_1k.rs

[^thread_limit]: On my Linux laptop I can spawn almost 19k threads before I hit
    this crash, but the Playground has tighter resource limits.

```
LINK: Playground playground://async_playground/threads_1k.rs
thread 'main' panicked at /rustc/3f5fd8dd41153bc5fdca9427e9e05...
failed to spawn thread: Os { code: 11, kind: WouldBlock, message:
"Resource temporarily unavailable" }
```

Threads are a fine way to run a few jobs in parallel, or even a few hundred,
but for various reasons they don't scale well beyond that.[^thread_pool] If we
want to run thousands of jobs at once, we need something different.

[^thread_pool]: A thread pool can be a good approach for CPU-intensive work,
    but when each jobs spends most of its time blocked on IO, the pool quickly
    runs out of worker threads, and there's [not enough parallelism to go
    around][rayon].

[rayon]: playground://async_playground/rayon.rs

## Async

Let's try the same thing with async/await. Part Two will go into all the
details, but for now I just want to type it out and run it on the Playground.
Our async `foo` function looks like this:[^tokio]

[^tokio]: Most of these examples will use the [Tokio](https://tokio.rs/)
    runtime and the [`futures`](https://docs.rs/futures/) support library.
    There are other options, but this is the most common way to do things.

```rust
async fn foo(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}
```

Making a few calls to `foo` one at a time looks like this:

```rust
LINK: Playground playground://async_playground/tokio.rs
foo(1).await;
foo(2).await;
foo(3).await;
```

Making several calls at the same time looks like this:

```rust
LINK: Playground playground://async_playground/tokio_10.rs
let mut futures = Vec::new();
for n in 1..=10 {
    futures.push(foo(n));
}
let joined_future = future::join_all(futures);
joined_future.await;
```

So far this might look like just another way of doing the same thing we were
doing with threads. But this works even if we bump it up to [a thousand
jobs][thousand_futures]. In fact, if we [comment out the prints and build in
release mode][million_futures], we can run _a million jobs_ at once.[^remember]

[thousand_futures]: playground://async_playground/tokio_1k.rs
[million_futures]: playground://async_playground/tokio_1m.rs?mode=release

[^remember]: For me this takes about two seconds, so it's spending about as
    much time working as it is sleeping. And remember this is on the
    Playground, with tight resource limits.

What exactly is a "future" though? Well, that's what Part Two is all about. For
now we'll just say that a future is what an async function returns. Let's
finish up by making a couple small mistakes with futures and seeing what
happens.

## Important Mistakes

We can get some hints about how async works if we start making some mistakes.
First let's try using [`std::thread::sleep`] instead of [`tokio::time::sleep`]
in our async function:

[`std::thread::sleep`]: https://doc.rust-lang.org/std/thread/fn.sleep.html
[`tokio::time::sleep`]: https://docs.rs/tokio/latest/tokio/time/fn.sleep.html

```rust
LINK: Playground playground://async_playground/tokio_blocking.rs
async fn foo(n: u64) {
    println!("start {n}");
    std::thread::sleep(Duration::from_secs(1)); // Oops!
    println!("end {n}");
}
```

Oh no! Everything is running one-at-a-time again! It's an easy mistake to make,
unfortunately.[^detect_blocking] But we can learn a lot about how a system
works by watching it fail, and what we're learning here is that all of the jobs
running "at the same time" in the async examples above were actually running on
a single thread. That's the magic of async. In the next part of this series,
we'll dive into all the nitty gritty details of how exactly this works.

[^detect_blocking]: There have been [attempts][async_std_proposal] to
    automatically detect and handle blocking in async functions, but that's led
    to [performance problems][tokio_blocking_note], and it hasn't been possible
    to handle [all cases][reddit_blocking_comment].

[async_std_proposal]: https://async.rs/blog/stop-worrying-about-blocking-the-new-async-std-runtime/
[reddit_blocking_comment]: https://www.reddit.com/r/rust/comments/ebfj3x/stop_worrying_about_blocking_the_new_asyncstd/fb4i9z5/
[tokio_blocking_note]: https://tokio.rs/blog/2020-04-preemption#a-note-on-blocking

TODO: This also doesn't work:

```rust
LINK: Playground playground://async_playground/tokio_serial.rs
for future in futures {
    future.await; // Oops!
}
```
