# Async Rust, part 1: Why are we here?
###### \[date]

- Part 2: How does it work?
- Part 3: Choose your own adventure

## Work, work

Async Rust is fun.[^block_links]

[^block_links]: Each code block in this article is a clickable link to a
    complete example on the [Rust Playground](https://play.rust-lang.org/).

```rust
LINK: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+work%28n%3A+u64%29+%7B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++work%281%29%3B%0A++++work%282%29%3B%0A++++work%283%29%3B%0A%7D%0A
use std::time::Duration;

fn work(n: u64) {
    std::thread::sleep(Duration::from_secs(1));
    println!("{n}");
}

fn main() {
    work(1);
    work(2);
    work(3);
}
```

Here's threads:

```rust
LINK: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+work%28n%3A+u64%29+%7B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+threads+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D100+%7B%0A++++++++threads.push%28std%3A%3Athread%3A%3Aspawn%28move+%7C%7C+work%28n%29%29%29%3B%0A++++%7D%0A++++for+thread+in+threads+%7B%0A++++++++thread.join%28%29.unwrap%28%29%3B%0A++++%7D%0A%7D%0A
fn main() {
    let mut threads = Vec::new();
    for n in 1..=3 {
        threads.push(std::thread::spawn(move || work(n)));
    }
    for thread in threads {
        thread.join().unwrap();
    }
}
```

And here's a lot of threads:

```rust
LINK: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+work%28n%3A+u64%29+%7B%0A++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+threads+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++threads.push%28std%3A%3Athread%3A%3Aspawn%28move+%7C%7C+work%28n%29%29%29%3B%0A++++%7D%0A++++for+thread+in+threads+%7B%0A++++++++thread.join%28%29.unwrap%28%29%3B%0A++++%7D%0A%7D%0A
thread 'main' panicked at /rustc/3f5fd8dd41153bc5fdca9427e9e05be2c767ba23/library/std/src/thread/mod.rs:698:29:
failed to spawn thread: Os { code: 11, kind: WouldBlock, message: "Resource temporarily unavailable" }
```

Now here's async:

```rust
LINK: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+work%28n%3A+u64%29+%7B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28work%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D%0A
use futures::future;
use std::time::Duration;

async fn work(n: u64) {
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("{n}");
}

#[tokio::main]
async fn main() {
    let mut futures = Vec::new();
    for n in 1..=1_000 {
        futures.push(work(n));
    }
    future::join_all(futures).await;
}
```
