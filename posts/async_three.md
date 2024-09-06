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
top-level future that's owned and polled by the main loop. Here we only have
one task, but there's nothing stopping us from having more than one. And if we
have a collection of tasks, we could even add to that collection at runtime.

This is what [`tokio::task::spawn`] does. We can rewrite our [original Tokio
example][tokio_10] using `spawn` instead of `join_all`:

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

`foo` is still an `async fn`, but otherwise this is very similar to [how we did
the same thing with threads][threads]. Like threads, but unlike ordinary
futures, tasks start running in the background as soon as you `spawn` them. A
common design pattern for network services is to have a main loop that listens
for new connections and spawns a thread to handle each connection. Async tasks
let us use the same design pattern[^futures_unordered] without the performance
overhead of threads.

[threads]: playground://async_playground/threads.rs

[^futures_unordered]: It's possible to do this with ordinary future
    combinators, but there are a couple downsides. Common idioms like
    [`join!`][join_macro] and [`select!`][select_macro] assume a static set of
    futures, so if you want to add futures dynamically you need fancy
    collections like [`FuturesUnordered`]. Runtimes like Tokio can also execute
    different tasks on different threads ("M:N threading"), but joined futures
    always run on the same thread.

[join_macro]: https://docs.rs/futures/latest/futures/macro.join.html
[select_macro]: https://docs.rs/futures/latest/futures/macro.select.html
[`FuturesUnordered`]: https://docs.rs/futures/latest/futures/stream/struct.FuturesUnordered.html

Building on [the main loop we wrote in Part Two][wakers], we can write our own
`spawn`. We'll do it in two steps. First we'll make space for multiple tasks in
the main loop, and then we'll implement the `spawn` function to add new ones.

[wakers]: playground://async_playground/wakers.rs

## Dyn

We already know how to poll many futures at once, because that's what we did
when [we implemented `JoinAll`][join_all]. How much of that code can we
copy/paste?

[join_all]: playground://async_playground/join.rs

One thing we need to change is the type of the `Vec`. Our `JoinAll` used
`Vec<Pin<Box<F>>>`,[^ignore_pin] where `F` was a generic type parameter, but
our main function doesn't have any type parameters. We also want our new `Vec`
to be able to hold futures of different types at the same time.[^same_thing]
The Rust feature we need here is ["dynamic trait objects"][dyn], or `dyn
Trait`.[^dyn] Let's start with a type alias so we don't have to write this more
than once:[^box]

[^ignore_pin]: We're still ignoring `Pin` for now, but `Box` is about to do
    some important work.

[^same_thing]: `JoinAll` can do this too, if you set `F` to the same type we're
    about to use.

[dyn]: https://doc.rust-lang.org/book/ch17-02-trait-objects.html

[^dyn]: `dyn Trait` isn't specific to async Rust. You might have seen it before
    in [error handling], where `Box<dyn Error>` is a catch-all type for the `?`
    operator. If you're coming from C++, `dyn Trait` is the closest thing Rust
    has to "`virtual` inheritance". If this is your first time seeing it, you
    might want to play with the [Rust by Example page for `dyn`][rbe_dyn].

[error handling]: https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html
[rbe_dyn]: https://doc.rust-lang.org/rust-by-example/trait/dyn.html

[^box]: This is where we start to care about the difference between `T` and
    `Box<T>`. Because `dyn Trait` is a ["dynamically sized type"][dst], we
    can't hold an object of that type directly in a local variable or a `Vec`
    element. We have to `Box` it.

[dst]: https://doc.rust-lang.org/book/ch19-04-advanced-types.html#dynamically-sized-types-and-the-sized-trait

```rust
type DynFuture = Pin<Box<dyn Future<Output = ()>>>;
```

Note that `DynFuture` doesn't have any type parameters. We can fit _any_ future
into this one type, as long as its `Output` is `()`. Now instead of building a
`join_future` in our `main` function, we'll build a `Vec<DynFuture>`, and we'll
start calling these futures "tasks":[^coercion]

[^coercion]: `Box::pin(foo(n))` is still a concrete future type, but pushing it
    into the `Vec<DynFuture>` "coerces" the concrete type to `dyn Future`.
    Specifically, it's an ["unsized coercion"].

["unsized coercion"]: https://doc.rust-lang.org/reference/type-coercions.html#unsized-coercions

```rust
LINK: Playground playground://async_playground/tasks_no_spawn.rs
let mut tasks: Vec<DynFuture> = Vec::new();
for n in 1..=10 {
    tasks.push(Box::pin(foo(n)));
}
```

We can manage the `Vec<DynFuture>` using `retain_mut` like `JoinAll` did,
removing futures from the `Vec` as soon as they're `Ready`. We do need to
restructure the `while` loop into a `loop`/`break` so that we can do all the
polling, then check whether we're done, then handle `Waker`s. Now it looks like
this:

```rust
LINK: Playground playground://async_playground/tasks_no_spawn.rs
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

    // Otherwise handle WAKERS and sleep as in Part Two...
    ...
```

This works fine, but it doesn't feel like we've accomplished much. Mostly we
just copy/pasted from `JoinAll` and tweaked the types. But we've laid some
important groundwork.

Before we move on, I want to highlight a couple differences between our tasks
and Tokio's tasks. The same way a regular Rust program exits when the main
thread is done, without waiting for background threads, a Tokio program exits
when the main task is done, without waiting for background tasks. But our main
loop continues until _all_ tasks are done. Our way is simpler, because we can
skip implementing [`JoinHandle`] for our tasks.[^lazy] Tokio also plumbs the
return value of a task through its `JoinHandle`, whereas we're assuming tasks
have no return value. These simplifications will work well enough for the rest
of this series.

[`JoinHandle`]: https://docs.rs/tokio/latest/tokio/task/struct.JoinHandle.html

[^lazy]: This is left as an exercise for the reader, as they say.

## Spawn

The `spawn` function will insert a future into the tasks `Vec`. How should it
access the `Vec`? It'd be nice if we could do the same thing we did with
`WAKERS` and make `TASKS` a global variable protected by a `Mutex`, but that's
not going to work this time. It worked before because the main loop only locked
`WAKERS` after it finished polling. But if `TASKS` is global, the main loop
will have to lock it while it's polling, and any task that calls `spawn` will
deadlock.

We can work around this by keeping `tasks` local to the main loop and making a
separate global called `NEW_TASKS`:[^vec_deque]

[^vec_deque]: We could use a `VecDeque` instead of a `Vec` if we wanted to poll
    tasks in FIFO order instead of LIFO order. Alternatively, using a [channel]
    would get rid of the `while let` footgun below, but creating a channel
    isn't `const`, so we'd need a [`OnceLock`] or similar to initialize the
    `static`.

[channel]: https://doc.rust-lang.org/rust-by-example/std_misc/channels.html
[`OnceLock`]: https://doc.rust-lang.org/stable/std/sync/struct.OnceLock.html

```rust
LINK: Playground playground://async_playground/compiler_errors/tasks_no_send_no_static.rs
static NEW_TASKS: Mutex<Vec<DynFuture>> = Mutex::new(Vec::new());

fn spawn<F: Future<Output = ()> + Send + 'static>(future: F) {
    NEW_TASKS.lock().unwrap().push(Box::pin(future));
}
```

Now the main loop can&hellip;wait that doesn't build:

```
LINK: Playground playground://async_playground/compiler_errors/tasks_no_send_no_static.rs
error[E0277]: `(dyn Future<Output = ()> + 'static)` cannot be sent between threads safely
    --> tasks_no_send_no_static.rs:43:19
     |
43   | static NEW_TASKS: Mutex<Vec<DynFuture>> = Mutex::new(Vec::new());
     |                   ^^^^^^^^^^^^^^^^^^^^^ `(dyn Future<Output = ()> + 'static)` cannot be sent between threads
     |
     = help: the trait `Send` is not implemented for `(dyn Future<Output = ()> + 'static)`, which is required by
             `Mutex<Vec<Pin<Box<(dyn Future<Output = ()> + 'static)>>>>: Sync`
```

Global variables in Rust have to be `Sync`, and `Mutex<T>` [is only `Sync` when
`T` is `Send`][mutex_sync].[^send_and_sync] `DynFuture` has to promise that
it's `Send`:

[mutex_sync]: https://doc.rust-lang.org/std/sync/struct.Mutex.html#impl-Sync-for-Mutex%3CT%3E

[^send_and_sync]: `Send` and `Sync` are the [thread safety marker
    traits][send_and_sync] in Rust.

[send_and_sync]: https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html

```rust
type DynFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
```

Ok, now the main loo&hellip;no it still doesn't build:

```
LINK: Playground playground://async_playground/compiler_errors/tasks_one_send_no_static.rs
error[E0277]: `F` cannot be sent between threads safely
  --> src/main.rs:46:36
   |
46 |     NEW_TASKS.lock().unwrap().push(Box::pin(future));
   |                                    ^^^^^^^^^^^^^^^^ `F` cannot be sent between threads safely
   |
   = note: required for the cast from `Pin<Box<F>>` to
           `Pin<Box<(dyn futures::Future<Output = ()> + std::marker::Send + 'static)>>`
```

Sure, `spawn` has to make the same promise:

```rust
fn spawn<F: Future<Output = ()> + Send>(future: F) { ... }
```

Happy yet? Nope:

```
LINK: Playground playground://async_playground/compiler_errors/tasks_no_static.rs
error[E0310]: the parameter type `F` may not live long enough
  --> src/main.rs:46:36
   |
46 |     NEW_TASKS.lock().unwrap().push(Box::pin(future));
   |                                    ^^^^^^^^^^^^^^^^
   |                                    |
   |                                    the parameter type `F` must be valid for the static lifetime...
   |                                    ...so that the type `F` will meet its required lifetime bounds
```

Global variables have the `'static` lifetime, meaning they don't hold pointers
to anything that could go away. Trait objects like `DynFuture` are `'static` by
default, but generic type parameters like `F` are not. If `spawn` wants to put
`F` in a global, it has to explicitly say that `F` is `'static`:

```rust
fn spawn<F: Future<Output = ()> + Send + 'static>(future: F) { ... }
```

It finally builds. That was a lot of ceremony just to make a global `Vec`, but
let's think about the meaning of what we wrote: Instead of a "`Vec` of
futures", `NEW_TASKS` is a "`Vec` of thread-safe futures which don't hold any
pointers that might become dangling." Rust doesn't have a garbage collector, so
dangling pointers would lead to memory corruption bugs, and it's nice that we
can just say we don't want those.[^thread_local]

[^thread_local]: The thread-safety requirement is arguably too strict, since
    we're not spawning any threads in this example. Rust doesn't have a way to
    say "I promise my program is single-threaded," but we could avoid the
    requirement by using a [`thread_local!`] instead of a `static`. In
    contrast, Tokio does use threads internally, so the `Send` requirement on
    [`tokio::task::spawn`] is unavoidable.

[`thread_local!`]: https://doc.rust-lang.org/std/macro.thread_local.html
[`tokio::task::spawn`]: https://docs.rs/tokio/latest/tokio/task/fn.spawn.html

Ok&hellip;_now_ the main loop can pop from `NEW_TASKS` and push into `tasks`.
It's not much extra code, but there are a couple pitfalls to watch out for, and
this time they're runtime problems instead of compiler errors. First, we need
to poll new tasks at least once as we collect them, rather than waiting until
the next iteration of the main loop, so that they get a chance to register
wakeups before we sleep. Second, we need to make sure that `NEW_TASKS` is
unlocked before we poll, or else we'll reintroduce the deadlock we're trying to
avoid.[^deadlock] Here's the expanded main loop, with new code in the middle:

[^deadlock]: A method chain like `.lock().unwrap().pop()` creates a temporary
    `MutexGuard` that lasts until the end of the current "statement". Normally
    that means the next semicolon. However, in the first line (the "scrutinee")
    of a `match`, an `if let`, or a `while let`, the current statement includes
    the entire following block. That rule fixes some common lifetime errors,
    but [it's a big footgun with locks][footgun].

[footgun]: https://fasterthanli.me/articles/a-rust-match-made-in-hell

```rust
LINK: Playground playground://async_playground/tasks_spawn.rs
loop {
    // Poll each task, removing any that are Ready.
    let is_pending = |task: &mut DynFuture| {
        task.as_mut().poll(&mut context).is_pending()
    };
    tasks.retain_mut(is_pending);

    // Collect new tasks, poll them, and keep the ones that are Pending.
    loop {
        let Some(mut task) = NEW_TASKS.lock().unwrap().pop() else {
            break;
        };
        // It's important that NEW_TASKS isn't locked here.
        if task.as_mut().poll(&mut context).is_pending() {
            tasks.push(task);
        }
    }

    // If there are no tasks left, we're done.
    if tasks.is_empty() {
        break;
    }

    // Otherwise handle WAKERS and sleep as in Part Two...
    ...
```

With all that in place, instead of hardcoding all our tasks in `main`, we can
define an `async_main` function let it `spawn` tasks:

```rust
LINK: Playground playground://async_playground/tasks_spawn.rs
async fn async_main() {
    // Note that this is different from Tokio.
    for n in 1..=10 {
        spawn(foo(n));
    }
}

fn main() {
    ...
    let mut tasks: Vec<DynFuture> = vec![Box::pin(async_main())];
    ...
```

It works! Because of how we push and pop `NEW_TASKS`, the order of prints is
different now. We could fix that, but let's keep it the way it is. It's a good
reminder that, like threads, tasks running at the same time can run in any
order.

So, did we do all that work just to have another way to spell `join_all`? Well,
yes and no. They are similar on the inside. But we're about to move beyond
sleeping and printing to look at real IO, and we're going to find that `spawn`
is exactly what we want when we're listening for network connections. Onward!
