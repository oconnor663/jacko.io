# Async Rust, Part Two: How does it work?
###### \[date]

- [Part One: What's in it for us?](async_one.html)
- Part Two: How does it work? (you are here)
- [Part Three: Choose your own adventure](async_three.html)

## Job

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Astruct+JobFuture+%7B%0A++++sleep_future%3A+Pin%3CBox%3Ctokio%3A%3Atime%3A%3ASleep%3E%3E%2C%0A++++n%3A+u64%2C%0A%7D%0A%0Aimpl+Future+for+JobFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28mut+self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+self.sleep_future.as_mut%28%29.poll%28context%29.is_pending%28%29+%7B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D+else+%7B%0A++++++++++++println%21%28%22%7B%7D%22%2C+self.n%29%3B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+job%28n%3A+u64%29+-%3E+JobFuture+%7B%0A++++let+sleep_future+%3D+tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++JobFuture+%7B%0A++++++++sleep_future%3A+Box%3A%3Apin%28sleep_future%29%2C%0A++++++++n%2C%0A++++%7D%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D
struct JobFuture {
    sleep_future: Pin<Box<tokio::time::Sleep>>,
    n: u64,
}

impl Future for JobFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if self.sleep_future.as_mut().poll(context).is_pending() {
            Poll::Pending
        } else {
            println!("{}", self.n);
            Poll::Ready(())
        }
    }
}

fn job(n: u64) -> JobFuture {
    let sleep_future = tokio::time::sleep(Duration::from_secs(1));
    JobFuture {
        sleep_future: Box::pin(sleep_future),
        n,
    }
}
```

## Join

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0Astruct+JoinFuture%3CF%3E+%7B%0A++++futures%3A+Vec%3CPin%3CBox%3CF%3E%3E%3E%2C%0A%7D%0A%0Aimpl%3CF%3A+Future%3E+Future+for+JoinFuture%3CF%3E+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28mut+self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++let+is_pending+%3D+%7Cfuture%3A+%26mut+Pin%3CBox%3CF%3E%3E%7C+%7B%0A++++++++++++future.as_mut%28%29.poll%28context%29.is_pending%28%29%0A++++++++%7D%3B%0A++++++++self.futures.retain_mut%28is_pending%29%3B%0A++++++++if+self.futures.is_empty%28%29+%7B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D+else+%7B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+join_all%3CF%3A+Future%3E%28futures%3A+Vec%3CF%3E%29+-%3E+JoinFuture%3CF%3E+%7B%0A++++JoinFuture+%7B%0A++++++++futures%3A+futures.into_iter%28%29.map%28Box%3A%3Apin%29.collect%28%29%2C%0A++++%7D%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++join_all%28futures%29.await%3B%0A%7D
struct JoinFuture<F> {
    futures: Vec<Pin<Box<F>>>,
}

impl<F: Future> Future for JoinFuture<F> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        let is_pending = |future: &mut Pin<Box<F>>| {
            future.as_mut().poll(context).is_pending()
        };
        self.futures.retain_mut(is_pending);
        if self.futures.is_empty() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

fn join_all<F: Future>(futures: Vec<F>) -> JoinFuture<F> {
    JoinFuture {
        futures: futures.into_iter().map(Box::pin).collect(),
    }
}
```

## Sleep

This version never wakes up:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Astruct+SleepFuture+%7B%0A++++wake_time%3A+Instant%2C%0A%7D%0A%0Aimpl+Future+for+SleepFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28self%3A+Pin%3C%26mut+Self%3E%2C+_%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+self.wake_time+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D+else+%7B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+sleep%28duration%3A+Duration%29+-%3E+SleepFuture+%7B%0A++++let+wake_time+%3D+Instant%3A%3Anow%28%29+%2B+duration%3B%0A++++SleepFuture+%7B+wake_time+%7D%0A%7D%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++sleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D
struct SleepFuture {
    wake_time: Instant,
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
        if self.wake_time <= Instant::now() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

fn sleep(duration: Duration) -> SleepFuture {
    let wake_time = Instant::now() + duration;
    SleepFuture { wake_time }
}
```

This version always wakes up, so the output is correct, but it burns the CPU:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Astruct+SleepFuture+%7B%0A++++wake_time%3A+Instant%2C%0A%7D%0A%0Aimpl+Future+for+SleepFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+self.wake_time+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D+else+%7B%0A++++++++++++context.waker%28%29.wake_by_ref%28%29%3B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+sleep%28duration%3A+Duration%29+-%3E+SleepFuture+%7B%0A++++let+wake_time+%3D+Instant%3A%3Anow%28%29+%2B+duration%3B%0A++++SleepFuture+%7B+wake_time+%7D%0A%7D%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++sleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D
fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
    if self.wake_time <= Instant::now() {
        Poll::Ready(())
    } else {
        context.waker().wake_by_ref();
        Poll::Pending
    }
}
```

The simplest way to avoid a busy wait is to spawn a thread to wake us up later.
If [each future spawned its own thread][same_crash], we'd run into the same
crash as in Part One. [A single background thread that collects wakers through
a channel][background_thread] will work, but that's a bit complicated...

[same_crash]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Astruct+SleepFuture+%7B%0A++++wake_time%3A+Instant%2C%0A%7D%0A%0Aimpl+Future+for+SleepFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+self.wake_time+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D+else+%7B%0A++++++++++++let+wake_time+%3D+self.wake_time%3B%0A++++++++++++let+waker+%3D+context.waker%28%29.clone%28%29%3B%0A++++++++++++std%3A%3Athread%3A%3Aspawn%28move+%7C%7C+%7B%0A++++++++++++++++std%3A%3Athread%3A%3Asleep%28wake_time.saturating_duration_since%28Instant%3A%3Anow%28%29%29%29%3B%0A++++++++++++++++waker.wake%28%29%3B%0A++++++++++++%7D%29%3B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+sleep%28duration%3A+Duration%29+-%3E+SleepFuture+%7B%0A++++let+wake_time+%3D+Instant%3A%3Anow%28%29+%2B+duration%3B%0A++++SleepFuture+%7B+wake_time+%7D%0A%7D%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++sleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D

[background_thread]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+crossbeam_channel%3A%3ARecvTimeoutError%3B%0Ause+crossbeam_channel%3A%3ASender%3B%0Ause+futures%3A%3Afuture%3B%0Ause+std%3A%3Acollections%3A%3ABTreeMap%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Async%3A%3ALazyLock%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%2C+Waker%7D%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Astatic+WAKER_SENDER%3A+LazyLock%3CSender%3C%28Instant%2C+Waker%29%3E%3E+%3D+LazyLock%3A%3Anew%28%7C%7C+%7B%0A++++let+%28sender%2C+receiver%29+%3D+crossbeam_channel%3A%3Aunbounded%3A%3A%3C%28Instant%2C+Waker%29%3E%28%29%3B%0A++++%2F%2F+Kick+off+the+waker+thread+the+first+time+this+sender+is+used.%0A++++std%3A%3Athread%3A%3Aspawn%28move+%7C%7C+%7B%0A++++++++%2F%2F+A+sorted+multimap+of+wake+times+and+wakers.+The+soonest+wake+time+will+be+first.%0A++++++++let+mut+tree+%3D+BTreeMap%3A%3A%3CInstant%2C+Vec%3CWaker%3E%3E%3A%3Anew%28%29%3B%0A++++++++loop+%7B%0A++++++++++++%2F%2F+Wait+to+receive+a+new+%28wake_time%2C+waker%29+pair.+If+we+already+have+one+or+more%0A++++++++++++%2F%2F+wakers%2C+wait+with+a+timeout%2C+waking+up+at+the+earliest+known+wake+time.+Otherwise%2C%0A++++++++++++%2F%2F+wait+with+no+timeout.%0A++++++++++++let+new_pair+%3D+if+let+Some%28%28first_wake_time%2C+_%29%29+%3D+tree.first_key_value%28%29+%7B%0A++++++++++++++++let+timeout+%3D+first_wake_time.saturating_duration_since%28Instant%3A%3Anow%28%29%29%3B%0A++++++++++++++++match+receiver.recv_timeout%28timeout%29+%7B%0A++++++++++++++++++++Ok%28%28wake_time%2C+waker%29%29+%3D%3E+Some%28%28wake_time%2C+waker%29%29%2C%0A++++++++++++++++++++Err%28RecvTimeoutError%3A%3ATimeout%29+%3D%3E+None%2C%0A++++++++++++++++++++Err%28RecvTimeoutError%3A%3ADisconnected%29+%3D%3E+unreachable%21%28%29%2C%0A++++++++++++++++%7D%0A++++++++++++%7D+else+%7B%0A++++++++++++++++match+receiver.recv%28%29+%7B%0A++++++++++++++++++++Ok%28%28wake_time%2C+waker%29%29+%3D%3E+Some%28%28wake_time%2C+waker%29%29%2C%0A++++++++++++++++++++Err%28_%29+%3D%3E+unreachable%21%28%29%2C%0A++++++++++++++++%7D%0A++++++++++++%7D%3B%0A++++++++++++%2F%2F+If+we+got+a+waker+pair+above+%28i.e.+we+didn%27t+time+out%29%2C+add+it+to+the+map.%0A++++++++++++if+let+Some%28%28wake_time%2C+waker%29%29+%3D+new_pair+%7B%0A++++++++++++++++tree.entry%28wake_time%29.or_default%28%29.push%28waker.clone%28%29%29%3B%0A++++++++++++%7D%0A++++++++++++%2F%2F+Loop+over+all+the+wakers+whose+wake+time+has+passed%2C+removing+them+from+the+map+and%0A++++++++++++%2F%2F+invoking+them.%0A++++++++++++while+let+Some%28entry%29+%3D+tree.first_entry%28%29+%7B%0A++++++++++++++++if+*entry.key%28%29+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++++++++++entry.remove%28%29.into_iter%28%29.for_each%28Waker%3A%3Awake%29%3B%0A++++++++++++++++%7D+else+%7B%0A++++++++++++++++++++break%3B%0A++++++++++++++++%7D%0A++++++++++++%7D%0A++++++++%7D%0A++++%7D%29%3B%0A++++sender%0A%7D%29%3B%0A%0Astruct+SleepFuture+%7B%0A++++wake_time%3A+Instant%2C%0A%7D%0A%0Aimpl+Future+for+SleepFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+self.wake_time+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D+else+%7B%0A++++++++++++let+waker_pair+%3D+%28self.wake_time%2C+context.waker%28%29.clone%28%29%29%3B%0A++++++++++++WAKER_SENDER.send%28waker_pair%29.unwrap%28%29%3B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+sleep%28duration%3A+Duration%29+-%3E+SleepFuture+%7B%0A++++let+wake_time+%3D+Instant%3A%3Anow%28%29+%2B+duration%3B%0A++++SleepFuture+%7B+wake_time+%7D%0A%7D%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++sleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D

It's more interesting to get the event loop to wake up at the right time. To do
that we need to rewrite it. Here's the minimal custom event loop:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+futures%3A%3Atask%3A%3Anoop_waker_ref%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Astruct+SleepFuture+%7B%0A++++wake_time%3A+Instant%2C%0A%7D%0A%0Aimpl+Future+for+SleepFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+self.wake_time+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D+else+%7B%0A++++++++++++context.waker%28%29.wake_by_ref%28%29%3B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+sleep%28duration%3A+Duration%29+-%3E+SleepFuture+%7B%0A++++let+wake_time+%3D+Instant%3A%3Anow%28%29+%2B+duration%3B%0A++++SleepFuture+%7B+wake_time+%7D%0A%7D%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++sleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D20_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++let+mut+main_future+%3D+Box%3A%3Apin%28future%3A%3Ajoin_all%28futures%29%29%3B%0A++++let+mut+context+%3D+Context%3A%3Afrom_waker%28noop_waker_ref%28%29%29%3B%0A++++while+main_future.as_mut%28%29.poll%28%26mut+context%29.is_pending%28%29+%7B%0A++++++++%2F%2F+Busy+loop%21%0A++++%7D%0A%7D
fn main() {
    let mut futures = Vec::new();
    for n in 1..=20_000 {
        futures.push(job(n));
    }
    let mut main_future = Box::pin(future::join_all(futures));
    let mut context = Context::from_waker(noop_waker_ref());
    while main_future.as_mut().poll(&mut context).is_pending() {
        // Busy loop!
    }
}
```

Now instead of busy looping, we can tell that loop how long to sleep. Let's add
a global:

```rust
static WAKERS: Mutex<BTreeMap<Instant, Vec<Waker>>> = Mutex::new(BTreeMap::new());
```

And have `SleepFuture` put wakers in there:

```rust
fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
    if self.wake_time <= Instant::now() {
        Poll::Ready(())
    } else {
        context.waker().wake_by_ref();
        Poll::Pending
    }
}
```

And finally the main polling loop can read from it:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+futures%3A%3Atask%3A%3Anoop_waker_ref%3B%0Ause+std%3A%3Acollections%3A%3ABTreeMap%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Async%3A%3AMutex%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%2C+Waker%7D%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Astatic+WAKERS%3A+Mutex%3CBTreeMap%3CInstant%2C+Vec%3CWaker%3E%3E%3E+%3D+Mutex%3A%3Anew%28BTreeMap%3A%3Anew%28%29%29%3B%0A%0Astruct+SleepFuture+%7B%0A++++wake_time%3A+Instant%2C%0A%7D%0A%0Aimpl+Future+for+SleepFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+self.wake_time+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D+else+%7B%0A++++++++++++let+mut+wakers_tree+%3D+WAKERS.lock%28%29.unwrap%28%29%3B%0A++++++++++++let+wakers_vec+%3D+wakers_tree.entry%28self.wake_time%29.or_default%28%29%3B%0A++++++++++++wakers_vec.push%28context.waker%28%29.clone%28%29%29%3B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+sleep%28duration%3A+Duration%29+-%3E+SleepFuture+%7B%0A++++let+wake_time+%3D+Instant%3A%3Anow%28%29+%2B+duration%3B%0A++++SleepFuture+%7B+wake_time+%7D%0A%7D%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++sleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++let+mut+main_future+%3D+Box%3A%3Apin%28future%3A%3Ajoin_all%28futures%29%29%3B%0A++++let+mut+context+%3D+Context%3A%3Afrom_waker%28noop_waker_ref%28%29%29%3B%0A++++while+main_future.as_mut%28%29.poll%28%26mut+context%29.is_pending%28%29+%7B%0A++++++++let+mut+wakers_tree+%3D+WAKERS.lock%28%29.unwrap%28%29%3B%0A++++++++let+next_wake+%3D+wakers_tree.keys%28%29.next%28%29.expect%28%22sleep+forever%3F%22%29%3B%0A++++++++std%3A%3Athread%3A%3Asleep%28next_wake.saturating_duration_since%28Instant%3A%3Anow%28%29%29%29%3B%0A++++++++while+let+Some%28entry%29+%3D+wakers_tree.first_entry%28%29+%7B%0A++++++++++++if+*entry.key%28%29+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++++++entry.remove%28%29.into_iter%28%29.for_each%28Waker%3A%3Awake%29%3B%0A++++++++++++%7D+else+%7B%0A++++++++++++++++break%3B%0A++++++++++++%7D%0A++++++++%7D%0A++++%7D%0A%7D
while main_future.as_mut().poll(&mut context).is_pending() {
    let mut wakers_tree = WAKERS.lock().unwrap();
    let next_wake = wakers_tree.keys().next().expect("sleep forever?");
    std::thread::sleep(next_wake.duration_since(Instant::now()));
    while let Some(entry) = wakers_tree.first_entry() {
        if *entry.key() <= Instant::now() {
            entry.remove().into_iter().for_each(Waker::wake);
        } else {
            break;
        }
    }
}
```
