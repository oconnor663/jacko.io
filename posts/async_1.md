# Async Rust, Part One: What's in it for us?
###### \[date]

- Part Two: How does it work?
- Part Three: Choose your own adventure

When we need a program to do many things at the same time, the most direct
approach is to use threads. This works well for a small-to-medium number of
jobs, but it runs into problems as the number of threads gets large.
Async/await can solve those problems. Here in Part 1 we'll demo those problems,
to get a sense of why we might want to learn async Rust.

Here's an example program that does three jobs, one at a time. Click the
Playground link on the right to watch it run:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+job%28n%3A+u64%29+%7B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++job%281%29%3B%0A++++job%282%29%3B%0A++++job%283%29%3B%0A%7D
use std::time::Duration;

fn job(n: u64) {
    std::thread::sleep(Duration::from_secs(1));
    println!("{n}");
}

fn main() {
    job(1);
    job(2);
    job(3);
}
```

## Threads

Three seconds is an awfully long time just to print three numbers, but if we
were reading them over a slow network, the results might not be so different.
If we put each job on its own thread, the program will run in one second
instead of three. We can even run a hundred jobs in one second:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+job%28n%3A+u64%29+%7B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+threads+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D100+%7B%0A++++++++threads.push%28std%3A%3Athread%3A%3Aspawn%28move+%7C%7C+job%28n%29%29%29%3B%0A++++%7D%0A++++for+thread+in+threads+%7B%0A++++++++thread.join%28%29.unwrap%28%29%3B%0A++++%7D%0A%7D
fn main() {
    let mut threads = Vec::new();
    for n in 1..=100 {
        threads.push(std::thread::spawn(move || job(n)));
    }
    for thread in threads {
        thread.join().unwrap();
    }
}
```

But if we want to run thousands of jobs, we start to run into trouble. Here's
what I see when I spawn a thousand threads on the Playground:[^thread_limit]

[^thread_limit]: On my Linux laptop I can spawn almost 19k threads before I hit
    this crash, but the Playground is more resource-constrained.

```
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+job%28n%3A+u64%29+%7B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+threads+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++threads.push%28std%3A%3Athread%3A%3Aspawn%28move+%7C%7C+job%28n%29%29%29%3B%0A++++%7D%0A++++for+thread+in+threads+%7B%0A++++++++thread.join%28%29.unwrap%28%29%3B%0A++++%7D%0A%7D
thread 'main' panicked at /rustc/3f5fd8dd41153bc5fdca9427e9e05...
failed to spawn thread: Os { code: 11, kind: WouldBlock, message:
"Resource temporarily unavailable" }
```

## Async

Here's the async version of the original example, running three jobs one at a
time:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++job%281%29.await%3B%0A++++job%282%29.await%3B%0A++++job%283%29.await%3B%0A%7D
use std::time::Duration;

async fn job(n: u64) {
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("{n}");
}

#[tokio::main]
async fn main() {
    job(1).await;
    job(2).await;
    job(3).await;
}
```

We can use `join_all` to run a large number of jobs at the same time:[^million]

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+work%28n%3A+u64%29+%7B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28work%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D%0A
#[tokio::main]
async fn main() {
    let mut futures = Vec::new();
    for n in 1..=1_000 {
        futures.push(work(n));
    }
    future::join_all(futures).await;
}
```

[^million]: In fact, if we [comment out the `println` and run in release mode][million], we
can run a _million_ of jobs at once.

[million]: https://play.rust-lang.org/?version=stable&mode=release&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Aasync+fn+job%28_n%3A+u64%29+%7B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++%2F%2F+Don%27t+print.+A+million+prints+is+too+much+output+for+the+Playground.%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++let+start+%3D+Instant%3A%3Anow%28%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A++++let+time+%3D+Instant%3A%3Anow%28%29+-+start%3B%0A++++println%21%28%22time%3A+%7B%3A.3%7D+seconds%22%2C+time.as_secs_f32%28%29%29%3B%0A%7D