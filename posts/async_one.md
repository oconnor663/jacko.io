# Async Rust, Part One: Why?
###### \[date]

- Part One: Why? (you are here)
- [Part Two: Futures](async_two.html)
- [Part Three: Tasks](async_three.html)
- [Part Four: IO](async_four.html)
- [Part Five: More!](async_five.html)

Async/await, or "async IO", is a new-ish language feature that lets us do more
than one thing at a time. Rust has had async/await since 2019.[^new_ish] It's
especially popular with websites and network services that handle many
connections at once,[^lots] because running lots of async "futures" or "tasks"
is more efficient than running lots of threads. This series of articles is
about what futures and tasks are and how they work.

[^new_ish]: For comparison C# added async/await in 2012, Python added it in
    2015, JS in 2017, and C++ in 2020.

[^lots]: "Many" here usually means ten thousand or more. This is sometimes
    called the ["C10K problem"][c10k], short for 10k clients or connections.

[c10k]: https://en.wikipedia.org/wiki/C10k_problem

At a high level, using threads means asking your OS and your hardware to do
things in parallel for you, but using async/await means reorganizing your own
code to do it yourself.[^concurrency] That's where the efficiency comes from,
but it also means the details of async tend to "leak" into your code, and that
makes it harder to learn. This series will be a details-first introduction to
async Rust, focused on translating async examples into ordinary Rust code that
we can execute and understand.

[^concurrency]: The famously confusing technical terms for this distinction are
    "parallelism" and "concurrency". They're important in programming language
    theory, because they abstract over many different languages and OSs. But
    since we're talking specifically about Rust, we can say "threads" and
    "futures".

Our examples will use lots of traits, generics, closures, and threads. I'll
assume that you've written some Rust before and that you've read [The Rust
Programming Language] or similar.[^ch_20] If not, this will be a bit of a
firehose, and you might want to refer to [Rust By Example] whenever you see
something new.[^books]

[The Rust Programming Language]: https://doc.rust-lang.org/book/
[Rust By Example]: https://doc.rust-lang.org/rust-by-example/

[^ch_20]: The multithreaded web server project in [Chapter 20] is particularly
    relevant.

[Chapter 20]: https://doc.rust-lang.org/book/ch20-00-final-project-a-web-server.html

[^books]: If you're the sort of programmer who doesn't like learning languages
    from books, consider [this advice from Bryan Cantrill][advice], who's just
    like you: "With Rust, you need to _learn_ it&hellip;buy the book, sit down,
    read the book in a quiet place&hellip;Rust rewards that."

[advice]: https://youtu.be/HgtRAbE1nBM?t=3913

Most of our async examples will use the [Tokio] async
"runtime".[^more_than_one] Building our own futures and tasks will help us
understand what a runtime is and what it does. For now, it's a library we use
to write async programs.

[Tokio]: https://tokio.rs/

[^more_than_one]: There are several async runtimes available in Rust, but the
    differences between them aren't important for this series. Tokio is the
    most popular and the most widely supported, and it's a good default.

Let's get started by doing more than one thing at a time with threads. This
will go smoothly at first, but soon we'll run into trouble.

## Threads

Here's an example function `foo` that takes a second to run:

```rust
fn foo(n: u64) {
    println!("start {n}");
    thread::sleep(Duration::from_secs(1));
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
fn main() {
    let mut thread_handles = Vec::new();
    for n in 1..=10 {
        thread_handles.push(thread::spawn(move || foo(n)));
    }
    for handle in thread_handles {
        handle.join().unwrap();
    }
}
```

Note that `join` here means "wait for the thread to finish". Threads start
running in the background as soon as we call `spawn`, so all of them are making
progress while we wait on the first one, and the rest of the calls to `join`
return quickly.

We can bump this example up to [a hundred threads][hundred_threads], and it
works just fine. But if we try to run [a thousand
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

Each thread uses a lot of memory,[^stack_space] so there's a limit on how many
threads we can run at once. It's harder to see on the Playground, but we can
also cause performance problems by [switching between lots of threads at
once][basketball_threads].[^basketball_demo] Threads are a fine way to run a
few jobs in parallel, or even a few hundred, but for various reasons they don't
scale well beyond that.[^thread_pool] If we want to run thousands of jobs at
once, we need something different.

[^stack_space]: Specifically, each thread allocates space for its "stack",
    which is 8&nbsp;MiB by default on Linux. The OS uses fancy tricks to
    allocate this space "lazily", but it's still a lot if we spawn thousands of
    threads.

[^basketball_demo]: This is a demo of passing "basketballs" back and forth
    among many threads, to show how thread switching overhead affects
    performance as the number of threads grows. It's longer and more
    complicated than the other examples in Part One, and it's ok to skip it.
    TODO: Is [this version][basketball_threads_orig] still blocked?

[basketball_threads]: https://play.rust-lang.org/?version=stable&mode=release&edition=2021&gist=fd952dba2f51ee595cd9ff6dbbc08c38
[basketball_threads_orig]: playground://async_playground/basketball_threads.rs?mode=release

[^thread_pool]: A thread pool can be a good approach for CPU-intensive work,
    but when each jobs spends most of its time blocked on IO, the pool quickly
    runs out of worker threads, and there's [not enough parallelism to go
    around][rayon].

[rayon]: playground://async_playground/rayon.rs

## Async

Let's try the same thing with async/await. Part Two will go into all the
details, but for now I just want to type it out and run it on the Playground.
Our async `foo` function looks like this:

```rust
async fn foo(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}
```

Making a few calls to `foo` one at a time looks like this:[^tokio_main]

[^tokio_main]: In Chapter Two and Chapter Three we'll implement a lot of what
    `#[tokio::main]` is doing ourselves. Until then we can just take it on
    faith that it's "the thing we put before `main` when we use Tokio."

```rust
LINK: Playground playground://async_playground/tokio.rs
HIGHLIGHT: 3-5
#[tokio::main]
async fn main() {
    foo(1).await;
    foo(2).await;
    foo(3).await;
}
```

Making several calls at the same time looks like this:

```rust
LINK: Playground playground://async_playground/tokio_10.rs
HIGHLIGHT: 3-8
#[tokio::main]
async fn main() {
    let mut futures = Vec::new();
    for n in 1..=10 {
        futures.push(foo(n));
    }
    let joined_future = future::join_all(futures);
    joined_future.await;
}
```

Despite its name, [`join_all`] is doing something very different from the
[`join`] method we used with threads. There joining meant waiting on something,
but here it means combining multiple "futures" together. We'll get to the
details in Part Two, but for now we can [add some more prints][tokio_10_dbg] to
see that can see `join_all` doesn't take any time, and none of `foo`s start
running until we `.await` the joined future.

[`join_all`]: https://docs.rs/futures/latest/futures/future/fn.join_all.html
[`join`]: https://doc.rust-lang.org/std/thread/struct.JoinHandle.html#method.join
[tokio_10_dbg]: playground://async_playground/tokio_10_dbg.rs

Unlike the threads example above, this works even if we bump it up to [a
thousand jobs][thousand_futures]. In fact, if we [comment out the prints and
build in release mode][million_futures], we can run _a million jobs_ at
once.[^remember]

[thousand_futures]: playground://async_playground/tokio_1k.rs
[million_futures]: playground://async_playground/tokio_1m.rs?mode=release

[^remember]: For me this takes about two seconds, so it's spending about as
    much time working as it is sleeping. And remember this is on the
    Playground, with tight resource limits. The [tasks
    version][basketball_tasks] of the basketball demo above is also much more
    efficient than the threads version, but it requires lots of concepts we
    haven't explained yet, so I don't want to focus on it.

[basketball_tasks]: playground://async_playground/basketball_tasks.rs?mode=release

## Important Mistakes

We can get some hints about how async works if we start making some mistakes.
First let's try using [`thread::sleep`] instead of [`tokio::time::sleep`]
in our async function:

[`thread::sleep`]: https://doc.rust-lang.org/std/thread/fn.sleep.html
[`tokio::time::sleep`]: https://docs.rs/tokio/latest/tokio/time/fn.sleep.html

```rust
LINK: Playground playground://async_playground/tokio_blocking.rs
async fn foo(n: u64) {
    println!("start {n}");
    thread::sleep(Duration::from_secs(1)); // Oops!
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
HIGHLIGHT: 7-9
#[tokio::main]
async fn main() {
    let mut futures = Vec::new();
    for n in 1..=10 {
        futures.push(foo(n));
    }
    for future in futures {
        future.await; // Oops!
    }
}
```

---

[Part Two: Futures →](async_two.html)
