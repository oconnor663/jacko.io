# Async Rust in Three Parts
###### \[DRAFT]

- Introduction (you are here)
  - [Threads](#threads)
  - [Async](#async)
  - [Important Mistakes](#important_mistakes)
- [Part One: Futures](async_futures.html)
- [Part Two: Tasks](async_tasks.html)
- [Part Three: IO](async_io.html)

Async/await, or "async IO", is a new-ish[^new_ish] language feature that lets
our programs do more than one thing at a time. It's sort of an alternative to
multithreading,[^threads] though Rust programs often use both. Async is popular
with websites and network services that handle many connections at once,[^lots]
because running lots of "futures" or "tasks" is more efficient than running
lots of threads. This series will be an introduction to futures, tasks, and
async IO.

[^new_ish]: Rust has had async/await since 2019. For comparison, C# added
    async/await in 2012, Python added it in 2015, JS in 2017, and C++ in 2020.

[^threads]: Throughout this series we'll compare examples using threads to
    examples that accomplish the same thing using async. If this is your first
    time using threads in any language, though, this approach might be more
    confusing than helpful, because you'll have to learn two things at once.
    For an introduction to threads, see [Chapter&nbsp;16][ch16] and
    [Chapter&nbsp;20][ch20] of [The Book].

[The Book]: https://doc.rust-lang.org/book
[ch16]: https://doc.rust-lang.org/book/ch16-00-concurrency.html
[ch20]: https://doc.rust-lang.org/book/ch20-00-final-project-a-web-server.html

[^lots]: "Many" here conventionally means ten thousand or more. This is
    sometimes called the ["C10K problem"][c10k], short for 10k clients or
    connections.

[c10k]: https://en.wikipedia.org/wiki/C10k_problem

If we think of threads as asking our OS and our hardware to do things in
parallel for us, then we can think of async/await as reorganizing our own code
to do that ourselves. This requires both new high-level concepts and also new
low-level machinery, and that combination can be overwhelming. This series will
mostly skip the concepts and jump straight into the machinery.[^concurrency]
We'll start by translating ("desugaring") async examples into ordinary Rust
that we can run and understand, and gradually we'll build our own async
"runtime".[^runtime]

[^concurrency]: For example, we're not going to talk about ["parallelism" vs
    "concurrency"][parallelism_vs_concurrency] at all.

[parallelism_vs_concurrency]: https://en.wikipedia.org/wiki/Concurrency_(computer_science)#/media/File:Parallelism_vs_concurrency.png

[^runtime]: For now, a "runtime" is a library or framework that we use to write
    async programs. Building our own futures, tasks, and IO will gradually make
    it clear what a runtime does for us.

In Rust maybe more than in other languages, async/await pulls together all the
tools in the language toolbox. In Part One alone we'll need enums, traits,
generics, closures, iterators, and smart pointers. I'll assume that you've
written some Rust before and that you've read [The Rust Programming
Language][The Book] ("The Book") or similar.[^ch20] If not, you might want to
refer to [Rust By Example] whenever you see something new.[^books]

[^ch20]: Again, the multithreaded web server project in [Chapter&nbsp;20][ch20]
    is especially relevant.

[Rust By Example]: https://doc.rust-lang.org/rust-by-example/

[^books]: If you're the sort of programmer who doesn't like learning languages
    from books, consider [this advice from Bryan Cantrill][advice], who's just
    like you: "With Rust, you need to _learn_ it&hellip;buy the book, sit down,
    read the book in a quiet place&hellip;Rust rewards that."

[advice]: https://youtu.be/HgtRAbE1nBM?t=3913

Let's get started by doing more than one thing at a time with threads. This
will go smoothly at first, but then we'll run into trouble.

## Threads

Here's an example function `foo` that takes a second to run:

```rust
LINK: Playground ## playground://async_playground/threads.rs
fn foo(n: u64) {
    println!("start {n}");
    thread::sleep(Duration::from_secs(1));
    println!("end {n}");
}
```

If we want to make several calls to `foo` at the same time, we can spawn a
thread for each one. Click on the Playground button to see that this takes one
second instead of ten:[^threads_order]

[^threads_order]: You'll probably also see the "start" and "end" prints appear
    out of order. One of the tricky things about threads is that we don't which
    one will finish first.

```rust
LINK: Playground ## playground://async_playground/threads.rs
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
LINK: Playground ## playground://async_playground/threads_1k.rs
thread 'main' panicked at /rustc/3f5fd8dd41153bc5fdca9427e9e05...
failed to spawn thread: Os { code: 11, kind: WouldBlock, message:
"Resource temporarily unavailable" }
```

Each thread uses a lot of memory,[^stack_space] so there's a limit on how many
threads we can spawn. It's harder to see on the Playground, but we can also
cause performance problems by [switching between lots of threads at
once][basketball_threads].[^basketball_demo] Threads are a fine way to run a
few jobs in parallel, or even a few hundred, but for various reasons they don't
scale well beyond that.[^thread_pool] If we want to run thousands of jobs at
once, we need something different.

[^stack_space]: In particular, each thread allocates space for its "stack",
    which is 8&nbsp;MiB by default on Linux. The OS uses fancy tricks to
    allocate this space "lazily", but it's still a lot if we spawn thousands of
    threads.

[^basketball_demo]: This is a demo of passing "basketballs" back and forth
    among many threads, to show how thread switching overhead affects
    performance as the number of threads grows. It's longer and more
    complicated than the other examples here, and it's ok to skip it.
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
Our async `foo` function looks like this:[^tokio]

```rust
LINK: Playground ## playground://async_playground/tokio.rs
async fn foo(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}
```

[^tokio]: The async examples in this introduction and in Part One will use the
    [Tokio] runtime. There are several async runtimes available in Rust, but
    the differences between them aren't important for this series. Tokio is the
    most popular and the most widely supported.

[Tokio]: https://tokio.rs/

Making a few calls to `foo` one at a time looks like this:[^tokio_main]

[^tokio_main]: In Parts Two and Three of this series, we'll implement a lot of
    what `#[tokio::main]` is doing. Until then we can just take it on faith
    that it's "the thing we put before `main` when we use Tokio."

```rust
LINK: Playground ## playground://async_playground/tokio.rs
#[tokio::main]
async fn main() {
    foo(1).await;
    foo(2).await;
    foo(3).await;
}
```

Making several calls at the same time looks like this:[^async_order]

[^async_order]: Unlike the version with threads above, you'll always see this
    version print its start messages in order, and you'll _usually_ see it
    print the end messages in order too. However, it's possible for the end
    messages to appear out of order, because [Tokio's timer implementation is
    complicated][new_tokio_timer].

[new_tokio_timer]: https://tokio.rs/blog/2018-03-timers

```rust
LINK: Playground ## playground://async_playground/tokio_10.rs
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
details in Part One, but for now we can [add some more prints][tokio_10_dbg] to
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
LINK: Playground ## playground://async_playground/tokio_blocking.rs
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
a single thread. That's the magic of async. In Part One, we'll dive into all
the nitty gritty details of how exactly this works.

[^detect_blocking]: There have been [attempts][async_std_proposal] to
    automatically detect and handle blocking in async functions, but that's led
    to [performance problems][tokio_blocking_note], and it hasn't been possible
    to handle [all cases][reddit_blocking_comment].

[async_std_proposal]: https://async.rs/blog/stop-worrying-about-blocking-the-new-async-std-runtime/
[reddit_blocking_comment]: https://www.reddit.com/r/rust/comments/ebfj3x/stop_worrying_about_blocking_the_new_asyncstd/fb4i9z5/
[tokio_blocking_note]: https://tokio.rs/blog/2020-04-preemption#a-note-on-blocking

We can also try awaiting each future in a loop, like how we used `join` with
threads above:

```rust
LINK: Playground ## playground://async_playground/tokio_serial.rs
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

This also doesn't work! What we're seeing is that futures don't automatically
do any work "in the background". Instead, they do their work when we await
them, so if we await them one-at-a-time, they'll do their work one-at-a-time.
Somehow, `future::join_all` is letting us await all of them at once. We'll see
how in Part One.

---

<div class="prev-next-arrows">
    <div class="space"> </div><div>
    <div><a href="async_futures.html">Part One: Futures →</a></div>
</div>
