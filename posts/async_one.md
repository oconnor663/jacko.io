# Async Rust, Part One: What's in it for us?
###### \[date]

- Part One: What's in it for us? (you are here)
- [Part Two: How does it work?](async_two.html)
- [Part Three: Choose your own adventure](async_three.html)

When we need a program to do many things at the same time, the most direct
approach is to use threads. This works well for a small-to-medium number of
jobs, but it runs into problems as the number of threads gets large.
Async/await can solve those problems. Here in Part 1 we'll demo those problems,
to get a sense of why we might want to learn async Rust.

Here's an example program that runs three jobs, one at a time. Click the
Playground link on the right to watch it run:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs%2C+one+at+a+time...%5Cn%22%29%3B%0A++++job%281%29%3B%0A++++job%282%29%3B%0A++++job%283%29%3B%0A%7D
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
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+threads+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++threads.push%28std%3A%3Athread%3A%3Aspawn%28move+%7C%7C+job%28n%29%29%29%3B%0A++++%7D%0A++++for+thread+in+threads+%7B%0A++++++++thread.join%28%29.unwrap%28%29%3B%0A++++%7D%0A%7D
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

[hundred_threads]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++println%21%28%22Run+a+hundred+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+threads+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D100+%7B%0A++++++++threads.push%28std%3A%3Athread%3A%3Aspawn%28move+%7C%7C+job%28n%29%29%29%3B%0A++++%7D%0A++++for+thread+in+threads+%7B%0A++++++++thread.join%28%29.unwrap%28%29%3B%0A++++%7D%0A%7D
[thousand_threads]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++println%21%28%22Run+a+thousand+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+threads+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++threads.push%28std%3A%3Athread%3A%3Aspawn%28move+%7C%7C+job%28n%29%29%29%3B%0A++++%7D%0A++++for+thread+in+threads+%7B%0A++++++++thread.join%28%29.unwrap%28%29%3B%0A++++%7D%0A%7D

[^thread_limit]: On my Linux laptop I can spawn almost 19k threads before I hit
    this crash, but the Playground is more resource-constrained.

```
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++println%21%28%22Run+a+thousand+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+threads+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++threads.push%28std%3A%3Athread%3A%3Aspawn%28move+%7C%7C+job%28n%29%29%29%3B%0A++++%7D%0A++++for+thread+in+threads+%7B%0A++++++++thread.join%28%29.unwrap%28%29%3B%0A++++%7D%0A%7D
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

[rayon]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++println%21%28%22Run+a+thousand+threads+on+a+thread+pool...%22%29%3B%0A++++rayon%3A%3Ascope%28%7Cscope%7C+%7B%0A++++++++for+n+in+1..%3D1_000+%7B%0A++++++++++++scope.spawn%28move+%7C_%7C+job%28n%29%29%3B%0A++++++++%7D%0A++++%7D%29%3B%0A%7D

## Async

Let's try the same thing with async/await. Part Two will go into all the
details, but for now I just want to type it out and run it on the Playground.
Our async `job` function looks like this:[^tokio]

[^tokio]: Most of these examples will use the [Tokio](https://tokio.rs/)
    runtime and the [`futures`](https://docs.rs/futures/) support library.
    There are other options, but this is the most common way to do things.

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs%2C+one+at+a+time...%5Cn%22%29%3B%0A++++job%281%29.await%3B%0A++++job%282%29.await%3B%0A++++job%283%29.await%3B%0A%0A++++println%21%28%22%5CnRun+three+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D
async fn job(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}
```

Running three jobs, one at a time looks like this:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs%2C+one+at+a+time...%5Cn%22%29%3B%0A++++job%281%29.await%3B%0A++++job%282%29.await%3B%0A++++job%283%29.await%3B%0A%0A++++println%21%28%22%5CnRun+three+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D
job(1).await;
job(2).await;
job(3).await;
```

And running three jobs at the same time looks like this:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs%2C+one+at+a+time...%5Cn%22%29%3B%0A++++job%281%29.await%3B%0A++++job%282%29.await%3B%0A++++job%283%29.await%3B%0A%0A++++println%21%28%22%5CnRun+three+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D
let mut futures = Vec::new();
for n in 1..=3 {
    futures.push(job(n));
}
future::join_all(futures).await;
```

This approach works even if we bump it up to [a thousand
jobs][thousand_futures]. In fact, if we [comment out the `println` and build in
release mode][million_futures], we can run a _million_ jobs at once.

[thousand_futures]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+a+thousand+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D
[million_futures]: https://play.rust-lang.org/?version=stable&mode=release&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Aasync+fn+job%28_n%3A+u64%29+%7B%0A++++%2F%2F+Don%27t+print.+A+million+prints+is+too+much+output+for+the+Playground.%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+a+million+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+start+%3D+Instant%3A%3Anow%28%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A++++let+time+%3D+Instant%3A%3Anow%28%29+-+start%3B%0A++++println%21%28%22time%3A+%7B%3A.3%7D+seconds%22%2C+time.as_secs_f32%28%29%29%3B%0A%7D

What is a "future"? Well, that's what Part Two is all about. For now, a future
is the thing that an async function returns.

## Mistake

We can get our first hint of how all of this works if we make a small mistake,
using [`std::thread::sleep`] instead of [`tokio::time::sleep`] in our async
function. Try it:

[`std::thread::sleep`]: https://doc.rust-lang.org/std/thread/fn.sleep.html
[`tokio::time::sleep`]: https://docs.rs/tokio/latest/tokio/time/fn.sleep.html

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B+%2F%2F+Oops%21%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs+at+the+same+time...%22%29%3B%0A++++println%21%28%22%5Cn...but+something%27s+not+right...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D
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
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs+at+the+same+time...%22%29%3B%0A++++println%21%28%22%5Cn...but+something%27s+not+right...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++for+future+in+futures+%7B%0A++++++++future.await%3B+%2F%2F+Oops%21%0A++++%7D%0A%7D
for future in futures {
    future.await; // Oops!
}
```
