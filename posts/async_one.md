# Async Rust, Part One: Why?
###### \[date]

- Part One: Why? (you are here)
- [Part Two: How?](async_two.html)
- [Part Three: More!](async_three.html)

When we need a program to do many things at the same time, the most direct
approach is to use threads. This works well for a small-to-medium number of
jobs, but it runs into problems as the number of threads gets large.
Async/await can solve those problems. Here in Part 1 we'll demo those problems,
to get a sense of why we might want to learn async Rust.

Here's an example program that runs three jobs, one at a time. Click the
Playground link on the right to watch it run:

```rust
LINK: Playground playground://async_playground/intro.rs
use std::time::Duration;

fn job(n: u64) {
    println!("start {n}");
    std::thread::sleep(Duration::from_secs(1));
    println!("end {n}");
}

fn main() {
    println!("Run three jobs, one at a time...\n");
    job(1);
    job(2);
    job(3);
}
```

## Threads

If we put each job on its own thread, the program runs in one second instead of
three:

```rust
LINK: Playground playground://async_playground/threads.rs
let mut threads = Vec::new();
for n in 1..=3 {
    threads.push(std::thread::spawn(move || job(n)));
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
    this crash, but the Playground is more resource-constrained.

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
Our async `job` function looks like this:[^tokio]

[^tokio]: Most of these examples will use the [Tokio](https://tokio.rs/)
    runtime and the [`futures`](https://docs.rs/futures/) support library.
    There are other options, but this is the most common way to do things.

```rust
LINK: Playground playground://async_playground/tokio.rs
async fn job(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}
```

Running three jobs, one at a time looks like this:

```rust
LINK: Playground playground://async_playground/tokio.rs
job(1).await;
job(2).await;
job(3).await;
```

And running three jobs at the same time looks like this:

```rust
LINK: Playground playground://async_playground/tokio.rs
let mut futures = Vec::new();
for n in 1..=3 {
    futures.push(job(n));
}
future::join_all(futures).await;
```

This approach works even if we bump it up to [a thousand
jobs][thousand_futures]. In fact, if we [comment out the `println` and build in
release mode][million_futures], we can run a _million_ jobs at once.

[thousand_futures]: playground://async_playground/tokio_1k.rs
[million_futures]: playground://async_playground/tokio_1m.rs?mode=release

What exactly is a "future" though? Well, that's what Part Two is all about. For
now we'll just say that a future is what an async function returns. Let's
finish up by making a couple small mistakes with futures and seeing what
happens.

## Mistakes

We can get our first hint of how all of this works if we make a small mistake,
using [`std::thread::sleep`] instead of [`tokio::time::sleep`] in our async
function. Try it:

[`std::thread::sleep`]: https://doc.rust-lang.org/std/thread/fn.sleep.html
[`tokio::time::sleep`]: https://docs.rs/tokio/latest/tokio/time/fn.sleep.html

```rust
LINK: Playground playground://async_playground/tokio_blocking.rs
async fn job(n: u64) {
    println!("start {n}");
    std::thread::sleep(Duration::from_secs(1)); // Oops!
    println!("end {n}");
}
```

Oh no! Everything is running one-at-a-time again! It's an easy mistake to make,
unfortunately. But we can learn a lot about how a system works by seeing how it
fails, and what we're learning here is that all of the jobs running "at the
same time" in the async examples above were actually running on a single
thread. That's the magic of async. In the next part of this series, we'll dive
into all the nitty gritty details of how exactly this works.

TODO: This also doesn't work:

```rust
LINK: Playground playground://async_playground/tokio_serial.rs
for future in futures {
    future.await; // Oops!
}
```
