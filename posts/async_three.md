# Async Rust, Part Three: Tasks
###### \[date]

- [Part One: Why?](async_one.html)
- [Part Two: Futures](async_two.html)
- Part Three: Tasks (you are here)
- [Part Four: IO](async_four.html)
- [Part Five: More!](async_five.html)

At the start of Part One, we said that async/await was about futures and tasks.
Part Two was firehose of details about futures, and now we can talk about
tasks. Luckily, we've already seen one, though we didn't call it that. The last
version of our main loop in Part Two looked like this:

```rust
LINK: Playground playground://async_playground/wakers.rs
while joined_future.as_mut().poll(&mut context).is_pending() {
    ...
}
```

That `joined_future` is the simplest possible example of a task. It's a
top-level future that's owned and polled by the main loop. That loop only
polled one task, but there's nothing stopping us from having more than one.
And if we had a collection of tasks, we could even add to that collection
at runtime.

This is what [`tokio::task::spawn`] does. We can rewrite our [original
Tokio example][tokio_10] using `spawn` instead of `join_all`:

[`tokio::task::spawn`]: https://docs.rs/tokio/latest/tokio/task/fn.spawn.html
[tokio_10]: playground://async_playground/tokio_10.rs

```rust
LINK: Playground playground://async_playground/tokio_tasks.rs
let mut task_handles = Vec::new();
for n in 1..=10 {
    task_handles.push(tokio::task::spawn(foo(n)));
}
for handle in task_handles {
    handle.await.unwrap();
}
```

`foo` is still an `async fn`, but otherwise this is very similar to [how we
did the same thing with threads][threads]. Like threads, but unlike
ordinary futures, tasks start running in the background as soon as you
`spawn` them. A common design pattern for network services is to have a
main loop that listens for new connections and spawns a thread to handle
each connection. Async tasks let us use the same design
pattern[^futures_unordered] without the performance overhead of threads.

[threads]: playground://async_playground/threads.rs

[^futures_unordered]: It's also possible to do this with ordinary future
    combinators, but there are a couple downsides. Common idioms like
    [`join!`][join_macro] and [`select!`][select_macro] assume a static set
    of futures, so if you want to add futures dynamically you need fancy
    collections like [`FuturesUnordered`]. Runtimes like Tokio can also
    execute different tasks on different threads ("M:N threading"), but
    joined futures always run on the same thread.

[join_macro]: https://docs.rs/futures/latest/futures/macro.join.html
[select_macro]: https://docs.rs/futures/latest/futures/macro.select.html
[`FuturesUnordered`]: https://docs.rs/futures/latest/futures/stream/struct.FuturesUnordered.html

Building on the main loop we wrote in Part Two, we can write our own
`spawn`. We'll do it in two steps. First we'll make space for multiple
futures in our main loop, and then we'll implement the `spawn` function to
add new ones.

## Tasks

If our main loop is going to poll a collection of futures, what type should
that collection be? In Part Two we used `Vec<Pin<Box<F>>>`,[^ignore_pin]
where `F` was a generic type parameter on `JoinAll`, but our main function
doesn't have any type parameters. We also want this collection to be able
to hold futures of different types at the same time. The Rust feature we
need here is ["dynamic trait objects"][dyn], or `dyn Trait`.[^dyn] Let's
start with a type alias so we don't have to write this more than
once:[^box]

[^ignore_pin]: We're still ignoring `Pin` for now, but we're about to see
    `Box` do some important work.

[dyn]: https://doc.rust-lang.org/book/ch17-02-trait-objects.html

[^dyn]: `dyn Trait` isn't specific to async Rust. You might have seen it before
    in [error handling], where `Box<dyn Error>` is a catch-all type for the `?`
    operator. If you're coming from C++, `dyn Trait` is the closest thing Rust
    has to "`virtual` inheritance". If this is your first time seeing it, you
    might want to look a the [Rust by Example entry for `dyn`][rbe_dyn].

[error handling]: https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html
[rbe_dyn]: https://doc.rust-lang.org/rust-by-example/trait/dyn.html

[^box]: This is where we start to care about the difference between `T` and
    `Box<T>`. Because `dyn Trait` is a ["dynamically sized type"][dst], we
    can't hold an object of that type directly in a local variable or a
    `Vec` element. We have to `Box` it.

[dst]: https://doc.rust-lang.org/book/ch19-04-advanced-types.html#dynamically-sized-types-and-the-sized-trait

```rust
type DynFuture = Pin<Box<dyn Future<Output = ()>>>;
```

We'll manage our `Vec<DynFuture>` using `retain_mut` like we did with
`JoinAll`, removing futures from the `Vec` as soon as they're `Ready`.
Here's a sketch out what our main loop will look like:

```rust
let mut tasks: Vec<DynFuture> = ...
loop {
    // Poll each task, removing any that are Ready.
    let is_pending = |task: &mut DynFuture| {
        task.as_mut().poll(&mut context).is_pending()
    };
    tasks.retain_mut(is_pending);

    // If there are no tasks left, we're done.
    if tasks.is_empty() {
        break;
    }

    // Handle WAKERS and sleeping as we did in Part Two.
    ...
}
```

[Here's a custom event loop with a growable list of tasks.][custom_tasks]

[custom_tasks]: playground://async_playground/tasks_spawn.rs
