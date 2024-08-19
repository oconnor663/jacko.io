# Async Rust, Part Two: How does it work?
###### \[date]

- [Part One: What's in it for us?](async_one.html)
- Part Two: How does it work? (you are here)
- [Part Three: Choose your own adventure](async_three.html)

In Part One we looked at [some async Rust code][part_one] without explaining
anything about how it worked. That left us with several mysteries: What's an
`async fn`, and what are the "futures" that they return? What is [`join_all`]
doing? How is [`tokio::time::sleep`] different from [`std::thread::sleep`]?
What does `#[tokio::main]` actually do?

[part_one]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs%2C+one+at+a+time...%5Cn%22%29%3B%0A++++job%281%29.await%3B%0A++++job%282%29.await%3B%0A++++job%283%29.await%3B%0A%0A++++println%21%28%22%5CnRun+three+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D
[`join_all`]: https://docs.rs/futures/latest/futures/future/fn.join_all.html
[`tokio::time::sleep`]: https://docs.rs/tokio/latest/tokio/time/fn.sleep.html
[`std::thread::sleep`]: https://doc.rust-lang.org/std/thread/fn.sleep.html

I think the best way to answer these questions is to translate each piece into
normal, non-async Rust code and stare at it for a while. We'll find that we can
replicate `job` and `join_all` without too much trouble, but writing our own
`sleep` is going to be a whole different story.[^universe] Here we go.

[^universe]: [If you wish to make an apple pie from scratch, you must first
    invent the universe.](https://youtu.be/BkHCO8f2TWs?si=gIfadwLGsvawJ3qn)

## Job

As a reminder, here's what `job` looked like when it was an `async fn`:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs%2C+one+at+a+time...%5Cn%22%29%3B%0A++++job%281%29.await%3B%0A++++job%282%29.await%3B%0A++++job%283%29.await%3B%0A%0A++++println%21%28%22%5CnRun+three+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D
async fn job(n: u64) {
    println!("start {n}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("end {n}");
}
```

We can rewrite it as a regular, non-async function that returns a future:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Astruct+JobFuture+%7B%0A++++n%3A+u64%2C%0A++++started%3A+bool%2C%0A++++sleep_future%3A+Pin%3CBox%3Ctokio%3A%3Atime%3A%3ASleep%3E%3E%2C%0A%7D%0A%0Aimpl+Future+for+JobFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28mut+self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+%21self.started+%7B%0A++++++++++++println%21%28%22start+%7B%7D%22%2C+self.n%29%3B%0A++++++++++++self.started+%3D+true%3B%0A++++++++%7D%0A++++++++if+self.sleep_future.as_mut%28%29.poll%28context%29.is_pending%28%29+%7B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D+else+%7B%0A++++++++++++println%21%28%22end+%7B%7D%22%2C+self.n%29%3B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+job%28n%3A+u64%29+-%3E+JobFuture+%7B%0A++++let+sleep_future+%3D+tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++JobFuture+%7B%0A++++++++n%2C%0A++++++++started%3A+false%2C%0A++++++++sleep_future%3A+Box%3A%3Apin%28sleep_future%29%2C%0A++++%7D%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs%2C+one+at+a+time...%5Cn%22%29%3B%0A++++job%281%29.await%3B%0A++++job%282%29.await%3B%0A++++job%283%29.await%3B%0A%0A++++println%21%28%22%5CnRun+three+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D
fn job(n: u64) -> JobFuture {
    let sleep_future = tokio::time::sleep(Duration::from_secs(1));
    JobFuture {
        n,
        started: false,
        sleep_future: Box::pin(sleep_future),
    }
}
```

You might want to open both versions on the Playground and look at them side by
side. Notice that the non-async version calls `tokio::time::sleep` but doesn't
`.await`[^compiler_error] the [`Sleep`] future that `sleep`
returns.[^uppercase] Instead it stores the `Sleep` future in a new
struct.[^box_pin] Here's the struct:

[^compiler_error]: It's a [compiler error] to use `.await` in a non-async
    function.

[compiler error]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Atime%3A%3ADuration%3B%0A%0Afn+main%28%29+%7B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A%7D

[`Sleep`]: https://docs.rs/tokio/latest/tokio/time/struct.Sleep.html

[^uppercase]: To repeat, `sleep` (lowercase) is an async function and `Sleep`
    (uppercase) is the future that it returns. It's confusing, but it's similar
    to how the [`map`] method on iterators returns an iterator called [`Map`].
    Futures and iterators have a lot in common, as we'll see.

[`map`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.map
[`Map`]: https://doc.rust-lang.org/std/iter/struct.Map.html

[^box_pin]: Wait a minute, what's `Box::pin`? Hold that thought for just a moment.

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Astruct+JobFuture+%7B%0A++++n%3A+u64%2C%0A++++started%3A+bool%2C%0A++++sleep_future%3A+Pin%3CBox%3Ctokio%3A%3Atime%3A%3ASleep%3E%3E%2C%0A%7D%0A%0Aimpl+Future+for+JobFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28mut+self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+%21self.started+%7B%0A++++++++++++println%21%28%22start+%7B%7D%22%2C+self.n%29%3B%0A++++++++++++self.started+%3D+true%3B%0A++++++++%7D%0A++++++++if+self.sleep_future.as_mut%28%29.poll%28context%29.is_pending%28%29+%7B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D+else+%7B%0A++++++++++++println%21%28%22end+%7B%7D%22%2C+self.n%29%3B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+job%28n%3A+u64%29+-%3E+JobFuture+%7B%0A++++let+sleep_future+%3D+tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++JobFuture+%7B%0A++++++++n%2C%0A++++++++started%3A+false%2C%0A++++++++sleep_future%3A+Box%3A%3Apin%28sleep_future%29%2C%0A++++%7D%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs%2C+one+at+a+time...%5Cn%22%29%3B%0A++++job%281%29.await%3B%0A++++job%282%29.await%3B%0A++++job%283%29.await%3B%0A%0A++++println%21%28%22%5CnRun+three+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D
struct JobFuture {
    n: u64,
    started: bool,
    sleep_future: Pin<Box<tokio::time::Sleep>>,
}

impl Future for JobFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        if !self.started {
            println!("start {}", self.n);
            self.started = true;
        }
        if self.sleep_future.as_mut().poll(context).is_pending() {
            Poll::Pending
        } else {
            println!("end {}", self.n);
            Poll::Ready(())
        }
    }
}
```

This is a lot to take in all at once. Before we even get started, I want to set
aside a couple things that we're not going to explain until later. The first is
the `Context` argument. We'll look at that below when we implement `sleep`. The
second is `Pin`. We'll come back to `Pin` in Part Three, but for now if you'll
forgive me, I'm going to bend the truth a little bit: `Pin` doesn't do
anything.[^lies] Think of `Pin<Box<T>>` as `Box<T>`,[^box] think of `Pin<&mut
T>` as a `&mut T`, and try not to think about `as_mut` at all.

[^lies]: As far as lies go, this one is surprisingly close to the truth.

[^box]: And if you haven't seen [`Box<T>`][box] before, that's just `T` "on the
    heap". The difference between the "stack" and the "heap" is an important
    part of systems programming, but for now we're skipping over all the
    details that aren't absolutely necessary. They'll be easier to remember
    once you know how the story ends.

[box]: https://doc.rust-lang.org/std/boxed/struct.Box.html

Ok, with those caveats out of the way, let's get into some details. We finally
have something more to say about what a "future" is. It's something that
implements the [`Future`] trait. Our `JobFuture` implements `Future`, so has a
`poll` method. The `poll` method asks a question: Is the future finished with
its work? If so, `poll` returns [`Poll::Ready`] with its `Output`.[^no_output]
If not, `poll` returns [`Poll::Pending`]. We can see that `JobFuture::poll`
won't return `Ready` until [`Sleep::poll`][Sleep] has returned `Ready`.

[`Future`]: https://doc.rust-lang.org/std/future/trait.Future.html
[`Poll::Ready`]: https://doc.rust-lang.org/std/task/enum.Poll.html
[`Poll::Pending`]: https://doc.rust-lang.org/std/task/enum.Poll.html
[Sleep]: https://docs.rs/tokio/latest/tokio/time/struct.Sleep.html

[^no_output]: Our original `job` function had no return value, so `JobFuture`
    has no `Output`. Rust represents no value with `()`, the empty tuple, also
    known as the "unit" type. Functions and futures with no return value are
    used for their side effects, like printing.

But `poll` isn't just a question. It's also where the work of the future
happens. When it's time for `job` to print, it's `JobFuture::poll` that does
the printing. So there's a compromise: `poll` does as much work as it can get
done quickly, but whenever it would need to wait or block, it returns `Pending`
instead.[^timing] That way the caller that's asking "Are you finished?" never
needs to wait for an answer. In return, the caller promises to call `poll`
again later to let it finish its work.

[^timing]: We can [add some timing and logging][timing] around the call to
    `Sleep::poll` to see that it always returns quickly too.

[timing]: https://play.rust-lang.org/?version=stable&mode=release&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Astruct+JobFuture+%7B%0A++++n%3A+u64%2C%0A++++started%3A+bool%2C%0A++++sleep_future%3A+Pin%3CBox%3Ctokio%3A%3Atime%3A%3ASleep%3E%3E%2C%0A%7D%0A%0Aimpl+Future+for+JobFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28mut+self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+%21self.started+%7B%0A++++++++++++println%21%28%22start+%7B%7D%22%2C+self.n%29%3B%0A++++++++++++self.started+%3D+true%3B%0A++++++++%7D%0A++++++++let+before+%3D+Instant%3A%3Anow%28%29%3B%0A++++++++let+poll_result+%3D+self.sleep_future.as_mut%28%29.poll%28context%29%3B%0A++++++++let+duration+%3D+Instant%3A%3Anow%28%29+-+before%3B%0A++++++++println%21%28%22Sleep%3A%3Apoll+returned+%7Bpoll_result%3A%3F%7D+in+%7Bduration%3A%3F%7D.%22%29%3B%0A++++++++if+poll_result.is_pending%28%29+%7B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D+else+%7B%0A++++++++++++println%21%28%22end+%7B%7D%22%2C+self.n%29%3B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+job%28n%3A+u64%29+-%3E+JobFuture+%7B%0A++++let+sleep_future+%3D+tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++JobFuture+%7B%0A++++++++n%2C%0A++++++++started%3A+false%2C%0A++++++++sleep_future%3A+Box%3A%3Apin%28sleep_future%29%2C%0A++++%7D%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs%2C+one+at+a+time...%5Cn%22%29%3B%0A++++job%281%29.await%3B%0A++++job%282%29.await%3B%0A++++job%283%29.await%3B%0A%0A++++println%21%28%22%5CnRun+three+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D

`JobFuture::poll` doesn't know how many times it's going to be called, and it
shouldn't print the "start" message more than once, so sets its `started` flag
to keep track.[^state_machine] It doesn't need to track whether it's printed
the "end" message, though, because after it returns `Ready` it won't be called
again.[^iterator]

[^state_machine]: In other words, `JobFuture` is a "state machine" with two
    states. In general, the number of states you need to track where you are in
    an `async fn` is the number of `.await` points plus one, but this gets
    complicated when there are branches or loops. The magic of async is that
    the compiler figures all this out for us, and we don't usually need to
    write our own `poll` functions like we're doing here.

[^iterator]: Technically it's a "logic error" to call `poll` again after it's
    returned `Ready`. It could do anything, including blocking or panicking.
    But because `poll` is not `unsafe`, it's not allowed to corrupt memory or
    commit other undefined behavior. It's exactly the same story as calling
    [`Iterator::next`] again after it's returned `None`.

[`Iterator::next`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#tymethod.next

We're starting to see how `std::thread::sleep` ruined our performance at the
end of Part One. If we put a blocking sleep in `JobFuture::poll` instead of
returning `Pending`, we get [exactly the same result][same_result].

[same_result]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Astruct+JobFuture+%7B%0A++++n%3A+u64%2C%0A%7D%0A%0Aimpl+Future+for+JobFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28self%3A+Pin%3C%26mut+Self%3E%2C+_context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++println%21%28%22start+%7B%7D%22%2C+self.n%29%3B%0A++++++++std%3A%3Athread%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29%3B+%2F%2F+Oops%21%0A++++++++println%21%28%22end+%7B%7D%22%2C+self.n%29%3B%0A++++++++Poll%3A%3AReady%28%28%29%29%0A++++%7D%0A%7D%0A%0Afn+job%28n%3A+u64%29+-%3E+JobFuture+%7B%0A++++JobFuture+%7B+n+%7D%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+a+thousand+jobs+at+the+same+time...%22%29%3B%0A++++println%21%28%22%5Cn...but+something%27s+not+right...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++future%3A%3Ajoin_all%28futures%29.await%3B%0A%7D

Onward!

## Join

It might seem like `join_all` is doing something much more magical than `job`,
but now that we've seen the moving parts of a future, it turns out we already
have everything we need. Let's make `join_all` into a non-async function
too:[^always_was]

[^always_was]: In fact it's [defined this way upstream][upstream].

[upstream]: https://docs.rs/futures-util/0.3.30/src/futures_util/future/join_all.rs.html#102-105

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0Astruct+JoinFuture%3CF%3E+%7B%0A++++futures%3A+Vec%3CPin%3CBox%3CF%3E%3E%3E%2C%0A%7D%0A%0Aimpl%3CF%3A+Future%3E+Future+for+JoinFuture%3CF%3E+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28mut+self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++let+is_pending+%3D+%7Cfuture%3A+%26mut+Pin%3CBox%3CF%3E%3E%7C+%7B%0A++++++++++++future.as_mut%28%29.poll%28context%29.is_pending%28%29%0A++++++++%7D%3B%0A++++++++self.futures.retain_mut%28is_pending%29%3B%0A++++++++if+self.futures.is_empty%28%29+%7B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D+else+%7B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+join_all%3CF%3A+Future%3E%28futures%3A+Vec%3CF%3E%29+-%3E+JoinFuture%3CF%3E+%7B%0A++++JoinFuture+%7B%0A++++++++futures%3A+futures.into_iter%28%29.map%28Box%3A%3Apin%29.collect%28%29%2C%0A++++%7D%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs%2C+one+at+a+time...%5Cn%22%29%3B%0A++++job%281%29.await%3B%0A++++job%282%29.await%3B%0A++++job%283%29.await%3B%0A%0A++++println%21%28%22%5CnRun+three+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++join_all%28futures%29.await%3B%0A%7D
fn join_all<F: Future>(futures: Vec<F>) -> JoinFuture<F> {
    JoinFuture {
        futures: futures.into_iter().map(Box::pin).collect(),
    }
}
```

Once again, the function doesn't do much,[^agreement] and all the interesting
work happens in the struct:

[^agreement]: Especially since we've agreed not to think too hard about `Box::pin`.

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3ADuration%3B%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++println%21%28%22start+%7Bn%7D%22%29%3B%0A++++tokio%3A%3Atime%3A%3Asleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22end+%7Bn%7D%22%29%3B%0A%7D%0A%0Astruct+JoinFuture%3CF%3E+%7B%0A++++futures%3A+Vec%3CPin%3CBox%3CF%3E%3E%3E%2C%0A%7D%0A%0Aimpl%3CF%3A+Future%3E+Future+for+JoinFuture%3CF%3E+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28mut+self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++let+is_pending+%3D+%7Cfuture%3A+%26mut+Pin%3CBox%3CF%3E%3E%7C+%7B%0A++++++++++++future.as_mut%28%29.poll%28context%29.is_pending%28%29%0A++++++++%7D%3B%0A++++++++self.futures.retain_mut%28is_pending%29%3B%0A++++++++if+self.futures.is_empty%28%29+%7B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D+else+%7B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+join_all%3CF%3A+Future%3E%28futures%3A+Vec%3CF%3E%29+-%3E+JoinFuture%3CF%3E+%7B%0A++++JoinFuture+%7B%0A++++++++futures%3A+futures.into_iter%28%29.map%28Box%3A%3Apin%29.collect%28%29%2C%0A++++%7D%0A%7D%0A%0A%23%5Btokio%3A%3Amain%5D%0Aasync+fn+main%28%29+%7B%0A++++println%21%28%22Run+three+jobs%2C+one+at+a+time...%5Cn%22%29%3B%0A++++job%281%29.await%3B%0A++++job%282%29.await%3B%0A++++job%283%29.await%3B%0A%0A++++println%21%28%22%5CnRun+three+jobs+at+the+same+time...%5Cn%22%29%3B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D3+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++join_all%28futures%29.await%3B%0A%7D
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
```

[`Vec::retain_mut`] does most of the heavy lifting. It takes a closure
argument, calls that closure on each element of the `Vec`, and deletes the
elements that returned `false`.[^algorithm] Here that means that we drop each
child future the first time it returns `Ready`, following the rule that we're
not supposed to `poll` them again after that.

[`Vec::retain_mut`]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.retain_mut

[^algorithm]: If we did this with a simple `for` loop, it would take
    O(n<sup>2</sup>) time, because `Vec::remove` is O(n). But `retain_mut` uses
    a clever algorithm that walks two pointers through the `Vec` and moves each
    element at most once.

[`Vec::remove`]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.remove

Having seen `JobFuture` above, there's really nothing else new here. From the
outside, it feels magical that we can run all these child futures at once, but
on the inside, all we're doing is calling `poll` on the elements of a `Vec`.
What makes this work is that each call to `poll` returns quickly, and that when
we return `Pending` we get called again later.

Note that we're taking a shortcut by ignoring the outputs of child
futures.[^payload] We can get away with that because we only use our version of
`join_all` with `job`, which has no return value. The real `join_all` returns a
`Vec<F::Output>`, and it need to do some more bookkeeping.

[^payload]: Specifically, when we call `.is_pending()` on the result of `poll`,
    we ignore any value that `Poll::Ready` might be carrying.

Onward!

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

## Wake

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

What we're seeing here is an important architectural fact about how async Rust
works. Futures "in the middle", like `JobFuture` and `JoinFuture`, don't really
need to "know" anything about how the event loop works. But "leaf" futures like
`SleepFuture` need to coordinate closely with the event loop to schedule
wakeups. This is why writing runtime-agnostic async libraries is hard.

## Loop

It's more interesting to get the event loop to wake up at the right time. To do
that we need to rewrite it. Here's the minimal custom event loop:

```rust
LINK: Playground https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+futures%3A%3Atask%3A%3Anoop_waker_ref%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%7D%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Astruct+SleepFuture+%7B%0A++++wake_time%3A+Instant%2C%0A%7D%0A%0Aimpl+Future+for+SleepFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+self.wake_time+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D+else+%7B%0A++++++++++++context.waker%28%29.wake_by_ref%28%29%3B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+sleep%28duration%3A+Duration%29+-%3E+SleepFuture+%7B%0A++++let+wake_time+%3D+Instant%3A%3Anow%28%29+%2B+duration%3B%0A++++SleepFuture+%7B+wake_time+%7D%0A%7D%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++sleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++let+mut+main_future+%3D+Box%3A%3Apin%28future%3A%3Ajoin_all%28futures%29%29%3B%0A++++let+mut+context+%3D+Context%3A%3Afrom_waker%28noop_waker_ref%28%29%29%3B%0A++++while+main_future.as_mut%28%29.poll%28%26mut+context%29.is_pending%28%29+%7B%0A++++++++%2F%2F+Busy+loop%21%0A++++%7D%0A%7D
fn main() {
    let mut futures = Vec::new();
    for n in 1..=1_000 {
        futures.push(job(n));
    }
    let mut main_future = Box::pin(future::join_all(futures));
    let mut context = Context::from_waker(noop_waker_ref());
    while main_future.as_mut().poll(&mut context).is_pending() {
        // Busy loop!
    }
}
```

NOTE HERE: Even though our loop is always polling, we still need the wakers. If
we don't call `wake()` our program never finishes.

Now instead of busy looping, we can tell that loop how long to sleep. Let's add
a global:[^thread_local]

[^thread_local]: It would be slightly more efficient to [use `thread_local!`
    and `RefCell` instead of `Mutex`][thread_local], but `Mutex` is the
    familiar way to make a global variable in safe Rust, and it's good enough.

[thread_local]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+futures%3A%3Atask%3A%3Anoop_waker_ref%3B%0Ause+std%3A%3Acell%3A%3ARefCell%3B%0Ause+std%3A%3Acollections%3A%3ABTreeMap%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%2C+Waker%7D%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Astd%3A%3Athread_local%21+%7B%0A++++static+WAKERS%3A+RefCell%3CBTreeMap%3CInstant%2C+Vec%3CWaker%3E%3E%3E+%3D+RefCell%3A%3Anew%28BTreeMap%3A%3Anew%28%29%29%3B%0A%7D%0A%0Astruct+SleepFuture+%7B%0A++++wake_time%3A+Instant%2C%0A%7D%0A%0Aimpl+Future+for+SleepFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+self.wake_time+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D+else+%7B%0A++++++++++++WAKERS.with_borrow_mut%28%7Cwakers_tree%7C+%7B%0A++++++++++++++++let+wakers_vec+%3D+wakers_tree.entry%28self.wake_time%29.or_default%28%29%3B%0A++++++++++++++++wakers_vec.push%28context.waker%28%29.clone%28%29%29%3B%0A++++++++++++++++Poll%3A%3APending%0A++++++++++++%7D%29%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+sleep%28duration%3A+Duration%29+-%3E+SleepFuture+%7B%0A++++let+wake_time+%3D+Instant%3A%3Anow%28%29+%2B+duration%3B%0A++++SleepFuture+%7B+wake_time+%7D%0A%7D%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++sleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++let+mut+main_future+%3D+Box%3A%3Apin%28future%3A%3Ajoin_all%28futures%29%29%3B%0A++++let+mut+context+%3D+Context%3A%3Afrom_waker%28noop_waker_ref%28%29%29%3B%0A++++while+main_future.as_mut%28%29.poll%28%26mut+context%29.is_pending%28%29+%7B%0A++++++++WAKERS.with_borrow_mut%28%7Cwakers_tree%7C+%7B%0A++++++++++++let+next_wake+%3D+wakers_tree.keys%28%29.next%28%29.expect%28%22sleep+forever%3F%22%29%3B%0A++++++++++++std%3A%3Athread%3A%3Asleep%28next_wake.duration_since%28Instant%3A%3Anow%28%29%29%29%3B%0A++++++++++++while+let+Some%28entry%29+%3D+wakers_tree.first_entry%28%29+%7B%0A++++++++++++++++if+*entry.key%28%29+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++++++++++entry.remove%28%29.into_iter%28%29.for_each%28Waker%3A%3Awake%29%3B%0A++++++++++++++++%7D+else+%7B%0A++++++++++++++++++++break%3B%0A++++++++++++++++%7D%0A++++++++++++%7D%0A++++++++%7D%29%3B%0A++++%7D%0A%7D

```rust
static WAKERS: Mutex<BTreeMap<Instant, Vec<Waker>>> =
    Mutex::new(BTreeMap::new());
```

And have `SleepFuture` put wakers in there:

```rust
fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
    if self.wake_time <= Instant::now() {
        Poll::Ready(())
    } else {
        let mut wakers_tree = WAKERS.lock().unwrap();
        let wakers_vec = wakers_tree.entry(self.wake_time).or_default();
        wakers_vec.push(context.waker().clone());
        Poll::Pending
    }
}
```

And finally the main polling loop can read from it:[^instant_only] [^hold_lock]

[^instant_only]: You might wonder why we bother calling `wake` here. Our
    top-level `Waker` is a no-op, we've already finished sleeping, and we're
    about to poll again, so what's the point? Well, it turns out that fancy
    combinators like [`JoinAll`] (not our simple version above, but the real
    one from [`futures`]) create a unique `Waker` internally for each of their
    children, and [they only poll children that have been awakened][skip_wake].
    This sort of thing is why [the docs for `Poll::Pending`][contract] say we
    must eventually wake the "current task".

[`JoinAll`]: https://docs.rs/futures/latest/futures/future/fn.join_all.html
[`futures`]: https://docs.rs/futures
[contract]: https://doc.rust-lang.org/std/task/enum.Poll.html#variant.Pending

[skip_wake]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+futures%3A%3Afuture%3B%0Ause+futures%3A%3Atask%3A%3Anoop_waker_ref%3B%0Ause+std%3A%3Acollections%3A%3ABTreeMap%3B%0Ause+std%3A%3Afuture%3A%3AFuture%3B%0Ause+std%3A%3Apin%3A%3APin%3B%0Ause+std%3A%3Async%3A%3AMutex%3B%0Ause+std%3A%3Atask%3A%3A%7BContext%2C+Poll%2C+Waker%7D%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Astatic+WAKERS%3A+Mutex%3CBTreeMap%3CInstant%2C+Vec%3CWaker%3E%3E%3E+%3D+Mutex%3A%3Anew%28BTreeMap%3A%3Anew%28%29%29%3B%0A%0Astruct+SleepFuture+%7B%0A++++wake_time%3A+Instant%2C%0A%7D%0A%0Aimpl+Future+for+SleepFuture+%7B%0A++++type+Output+%3D+%28%29%3B%0A%0A++++fn+poll%28self%3A+Pin%3C%26mut+Self%3E%2C+context%3A+%26mut+Context%29+-%3E+Poll%3C%28%29%3E+%7B%0A++++++++if+self.wake_time+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++Poll%3A%3AReady%28%28%29%29%0A++++++++%7D+else+%7B%0A++++++++++++let+mut+wakers_tree+%3D+WAKERS.lock%28%29.unwrap%28%29%3B%0A++++++++++++let+wakers_vec+%3D+wakers_tree.entry%28self.wake_time%29.or_default%28%29%3B%0A++++++++++++wakers_vec.push%28context.waker%28%29.clone%28%29%29%3B%0A++++++++++++Poll%3A%3APending%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+sleep%28duration%3A+Duration%29+-%3E+SleepFuture+%7B%0A++++let+wake_time+%3D+Instant%3A%3Anow%28%29+%2B+duration%3B%0A++++SleepFuture+%7B+wake_time+%7D%0A%7D%0A%0Aasync+fn+job%28n%3A+u64%29+%7B%0A++++sleep%28Duration%3A%3Afrom_secs%281%29%29.await%3B%0A++++println%21%28%22%7Bn%7D%22%29%3B%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+futures+%3D+Vec%3A%3Anew%28%29%3B%0A++++for+n+in+1..%3D1_000+%7B%0A++++++++futures.push%28job%28n%29%29%3B%0A++++%7D%0A++++let+mut+main_future+%3D+Box%3A%3Apin%28future%3A%3Ajoin_all%28futures%29%29%3B%0A++++let+mut+context+%3D+Context%3A%3Afrom_waker%28noop_waker_ref%28%29%29%3B%0A++++while+main_future.as_mut%28%29.poll%28%26mut+context%29.is_pending%28%29+%7B%0A++++++++let+mut+wakers_tree+%3D+WAKERS.lock%28%29.unwrap%28%29%3B%0A++++++++let+next_wake+%3D+wakers_tree%0A++++++++++++.keys%28%29%0A++++++++++++.next%28%29%0A++++++++++++.expect%28%22OOPS%21+The+main+future+is+Pending+but+there%27s+no+wake+time.%22%29%3B%0A++++++++std%3A%3Athread%3A%3Asleep%28next_wake.duration_since%28Instant%3A%3Anow%28%29%29%29%3B%0A++++++++while+let+Some%28entry%29+%3D+wakers_tree.first_entry%28%29+%7B%0A++++++++++++if+*entry.key%28%29+%3C%3D+Instant%3A%3Anow%28%29+%7B%0A++++++++++++++++%2F%2F+OOPS%3A+Skip+invoking+the+wakers.+This+eventually+leads+to+a+panic+above%2C+because%0A++++++++++++++++%2F%2F+JoinAll+will+return+Pending+without+polling+any+of+its+children+a+second+time.%0A++++++++++++++++%2F%2F+NOTE%3A+As+of+futures+v0.3.30%2C+you+can+%22fix%22+this+by+reducing+the+number+of+jobs%0A++++++++++++++++%2F%2F+to+30+or+fewer.+Below+that+threshold%2C+JoinAll+falls+back+to+a+simple%0A++++++++++++++++%2F%2F+implementation+that+always+polls+its+children.%0A++++++++++++++++%2F%2F+https%3A%2F%2Fdocs.rs%2Ffutures%2F0.3.30%2Ffutures%2Ffuture%2Ffn.join_all.html%23see-also%0A++++++++++++++++%2F%2F+https%3A%2F%2Fdocs.rs%2Ffutures-util%2F0.3.30%2Fsrc%2Ffutures_util%2Ffuture%2Fjoin_all.rs.html%2335%0A++++++++++++++++entry.remove%28%29%3B%0A++++++++++++%7D+else+%7B%0A++++++++++++++++break%3B%0A++++++++++++%7D%0A++++++++%7D%0A++++%7D%0A%7D

[^hold_lock]: We're holding the `WAKERS` lock while we sleep here, which is a
    little sketchy, but it doesn't matter in this single-threaded example. A
    real multithreaded runtime would use [`std::thread::park_timeout`] or
    similar instead of sleeping, so that other threads could wake it up early.

[`std::thread::park_timeout`]: https://doc.rust-lang.org/std/thread/fn.park_timeout.html

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
