# Async Rust, Part Two: Tasks
###### \[DRAFT]

- [Introduction](async_intro.html)
- [Part One: Futures](async_futures.html)
- Part Two: Tasks (you are here)
  - [Dyn](#dyn)
  - [Spawn](#spawn)
  - [JoinHandle](#joinhandle)
  - [Waker](#waker)
- [Part Three: IO](async_io.html)

In the introduction we said that async/await was about futures and tasks. Part
One was firehose of details about futures, and now we can talk about tasks.
Luckily, we've already seen one, though we didn't call it that. The last
version of our main loop in Part One looked like this:

```rust
LINK: Playground ## playground://async_playground/wakers.rs
HIGHLIGHT: 1,4,6
let mut joined_future = Box::pin(future::join_all(futures));
let waker = futures::task::noop_waker();
let mut context = Context::from_waker(&waker);
while joined_future.as_mut().poll(&mut context).is_pending() {
    ...
}
```

That `joined_future` is the simplest possible example of a task. It's a
top-level future that's owned and polled by the main loop. Here we only have
one task, but there's nothing stopping us from having more than one. And if we
had a collection of tasks, we could even add to that collection at runtime.

That's what [`tokio::task::spawn`] does. We can rewrite our [original Tokio
example][tokio_10] using `spawn` instead of `join_all`:

[`tokio::task::spawn`]: https://docs.rs/tokio/latest/tokio/task/fn.spawn.html
[tokio_10]: playground://async_playground/tokio_10.rs

```rust
LINK: Playground ## playground://async_playground/tokio_tasks.rs
HIGHLIGHT: 3-9
#[tokio::main]
async fn main() {
    let mut task_handles = Vec::new();
    for n in 1..=10 {
        task_handles.push(tokio::task::spawn(foo(n)));
    }
    for handle in task_handles {
        handle.await.unwrap();
    }
}
```

`foo` is still an `async fn`, but otherwise this is very similar to [how we did
the same thing with threads][threads]. Like threads, but unlike ordinary
futures, tasks start running in the background as soon as we `spawn` them. It's
common in network services to have a main loop that listens for new connections
and spawns a thread to handle each connection. Async tasks let us use the same
pattern without the performance overhead of threads.[^futures_unordered]

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

Building on [the main loop we wrote in Part One][wakers], we can write our own
`spawn`. We'll do it in three steps: First we'll make space for multiple tasks
in the main loop, then we'll write the `spawn` function to add new tasks, and
finally we'll implement [`JoinHandle`] to let one task wait for another.

[wakers]: playground://async_playground/wakers.rs
[`JoinHandle`]: https://docs.rs/tokio/latest/tokio/task/struct.JoinHandle.html

## Dyn

We already know how to poll many futures at once, because that's what we did
when [we implemented `JoinAll`][join_all]. How much of that code can we
copy/paste?

[join_all]: playground://async_playground/join.rs

One thing we need to change is the type of the futures `Vec`. Our `JoinAll`
used `Vec<Pin<Box<F>>>`,[^ignore_pin] where `F` was a generic type parameter,
but our main function doesn't have any type parameters. We also want the new
`Vec` to be able to hold futures of different types at the same
time.[^same_thing] The Rust feature we need here is ["dynamic trait
objects"][dyn], or `dyn Trait`.[^dyn] Let's start with a type alias so we don't
have to write this more than once:[^box]

[^ignore_pin]: We're still not paying much attention to `Pin`, but `Box` is
    about to do some important work.

[^same_thing]: `JoinAll` can do this too, if you set `F` to the same type we're
    about to use.

[dyn]: https://doc.rust-lang.org/book/ch17-02-trait-objects.html

[^dyn]: `dyn Trait` isn't specific to async. You might have seen it before in
    [error handling], where `Box<dyn Error>` is a catch-all type for the `?`
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

Note that `DynFuture` doesn't have type parameters. We can fit _any_ boxed
future into this _one_ type, as long as its `Output` is `()`. Now instead of
building a `join_future` in our `main` function, we'll build a
`Vec<DynFuture>`, and we'll start calling these futures "tasks":[^coercion]

[^coercion]: `Box::pin(foo(n))` is still a concrete future type, but pushing it
    into the `Vec<DynFuture>` "coerces" the concrete type to `dyn Future`.
    Specifically, it's an ["unsized coercion"].

["unsized coercion"]: https://doc.rust-lang.org/reference/type-coercions.html#unsized-coercions

```rust
LINK: Playground ## playground://async_playground/tasks_no_spawn.rs
HIGHLIGHT: 2-5
fn main() {
    let mut tasks: Vec<DynFuture> = Vec::new();
    for n in 1..=10 {
        tasks.push(Box::pin(foo(n)));
    }
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    ...
```

We can manage the `Vec<DynFuture>` using `retain_mut` like `JoinAll` did,
removing futures from the `Vec` as soon as they're `Ready`. We need to
rearrange the `while` loop into a `loop`/`break` so that we can do all the
polling, then check whether we're done, then handle `Waker`s. Now it looks like
this:

```rust
LINK: Playground ## playground://async_playground/tasks_no_spawn.rs
HIGHLIGHT: 3-16
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
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

        // Otherwise handle WAKERS and sleep as in Part One...
        ...
```

This works fine, though it might not feel like we've accomplished much. Mostly
we just copy/pasted from `JoinAll` and tweaked the types. But we've laid some
important groundwork.

Note that the behavior of this loop is somewhat different from how tasks work
in Tokio. The same way a regular Rust program exits when the main _thread_ is
done without waiting for background threads, a Tokio program also exits when
the main _task_ is done without waiting for background tasks. However, this
version of our main loop continues until _all_ tasks are done. It also assumes
that tasks have no return value. We'll fix both of these things when we get to
`JoinHandle` below, but let's do `spawn` first.

## Spawn

The `spawn` function is supposed to insert another future into the tasks `Vec`.
How should it access the `Vec`? It would be convenient if we could do the same
thing we did with `WAKERS` and make `TASKS` a global variable protected by a
`Mutex`, but that's not going to work this time. Our main loop only needs to
lock `WAKERS` after it's finished polling, but if we made `TASKS` global, then
the main loop would lock it _during_ polling, and any task that called `spawn`
would deadlock.

We'll work around that by maintaining two separate lists. We'll keep the
`tasks` list where it is, local to the main loop, and we'll add a global list
called `NEW_TASKS`. The `spawn` function will append to
`NEW_TASKS`:[^vec_deque]

[^vec_deque]: We could use a `VecDeque` instead of a `Vec` if we wanted to poll
    tasks in FIFO order instead of LIFO order. We could also use a [channel],
    which as an added benefit would would get rid of the `while let` footgun
    below. Creating a channel isn't `const`, however, so we'd need a
    [`OnceLock`] or similar to initialize the `static`.

[channel]: https://doc.rust-lang.org/rust-by-example/std_misc/channels.html
[`OnceLock`]: https://doc.rust-lang.org/stable/std/sync/struct.OnceLock.html

```rust
LINK: Playground ## playground://async_playground/compiler_errors/tasks_no_send_no_static.rs
static NEW_TASKS: Mutex<Vec<DynFuture>> = Mutex::new(Vec::new());

fn spawn<F: Future<Output = ()>>(future: F) {
    NEW_TASKS.lock().unwrap().push(Box::pin(future));
}
```

Now the main loop can&hellip;wait that doesn't build:

```
LINK: Playground ## playground://async_playground/compiler_errors/tasks_no_send_no_static.rs
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
    traits][send_and_sync] in Rust. Another way of putting this requirement is
    that a `Mutex` is only safe to share with other threads if the obejct
    inside of it is safe to move to other threads.

[send_and_sync]: https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html

```rust
type DynFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
```

Ok, now the&hellip;nope it still doesn't build:

```
LINK: Playground ## playground://async_playground/compiler_errors/tasks_one_send_no_static.rs
error[E0277]: `F` cannot be sent between threads safely
  --> src/main.rs:46:36
   |
46 |     NEW_TASKS.lock().unwrap().push(Box::pin(future));
   |                                    ^^^^^^^^^^^^^^^^ `F` cannot be sent between threads safely
   |
   = note: required for the cast from `Pin<Box<F>>` to
           `Pin<Box<(dyn futures::Future<Output = ()> + std::marker::Send + 'static)>>`
```

Fair enough, `spawn` has to make the same promise:

```rust
fn spawn<F: Future<Output = ()> + Send>(future: F) { ... }
```

Happy yet? Nope:

```
LINK: Playground ## playground://async_playground/compiler_errors/tasks_no_static.rs
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
default, but type parameters like `F` are not. If `spawn` wants to put `F` in a
global, it also has to promise that `F` is `'static`:[^spawn_vs_join]

[^spawn_vs_join]: Note that `join_all` in Part One did not have this `'static`
    requirement. We can have multiple concurrent futures borrowing local
    variables, but we can't do the same with tasks. On the other hand, it's
    possible to run different tasks on different threads, as Tokio does by
    default, but we can't do that with non-`'static` futures. It would be nice
    if there was some task equivalent of [`std::thread::scope`], but that turns
    out to be an [open research question].

[`std::thread::scope`]: https://doc.rust-lang.org/stable/std/thread/fn.scope.html
[open research question]: https://without.boats/blog/the-scoped-task-trilemma/

```rust
fn spawn<F: Future<Output = ()> + Send + 'static>(future: F) { ... }
```

Finally it builds. That was a lot of ceremony just to make a global `Vec`, but
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
this time they're runtime bugs instead of compiler errors. First, we have to
poll new tasks as we collect them, rather than waiting until the next iteration
of the main loop, so they get a chance to register wakeups before we
sleep.[^wakeups] Second, we have to make sure that `NEW_TASKS` is unlocked
before we poll, or else we'll recreate the same deadlock we were trying to
avoid.[^deadlock] Here's the expanded main loop:

[^wakeups]: We'd notice this mistake immediately below, after we added the
    `async_main` function that calls `spawn`. If our main loop didn't poll
    those new tasks before it tried to read the next wakeup time, then there
    wouldn't be a wakeup time, and it would panic.

[^deadlock]: Unfortunately this is an easy mistake to make. A method chain like
    `.lock().unwrap().pop()` creates a [`MutexGuard`] that lasts until the end
    of the current ["temporary scope"][rule]. In this example as written,
    that's the semicolon after the `let else`. But if we combined the inner
    `loop` and the `let else` into a `while let`, or if we replaced the
    `let else` with an `if let`, the guard would last until the end of the
    _block_, and we'd still be holding the lock when we called `poll`. If we
    made this mistake, and if we also made `foo` call `spawn` before its first
    `.await`, this example would deadlock. This is [an unfortunate footgun with
    `Mutex`][footgun]. There's [a Clippy lint for it][lint], but as of Rust
    1.81 it's still disabled by default.
    <br>
    &emsp;&emsp;The [formal rule for this behavior][rule] is that the first
    part of an `if` or `while` expression (the "condition") is a "temporary
    scope", but the first part of a `match`, `if let`, or `while let`
    expression (the "scrutinee") is not. This rule is necessary for matching on
    borrowing methods like [`Vec::first`] or [`String::trim`], but it's
    unnecessary and counterintuitive with methods like [`Vec::pop`] or
    [`String::len`] that return owned values. It might be nice if Rust dropped
    temporaries as soon as possible, but then drop timing would depend on
    borrow checker analysis, which isn't generally stable. Some Rust compilers
    have even [skipped borrow checking entirely][mrustc], since correct
    programs can be compiled without it.

[`MutexGuard`]: https://doc.rust-lang.org/stable/std/sync/struct.MutexGuard.html
[`Vec::first`]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.first
[`String::trim`]: https://doc.rust-lang.org/std/string/struct.String.html#method.trim
[footgun]: https://fasterthanli.me/articles/a-rust-match-made-in-hell
[rule]: https://doc.rust-lang.org/reference/destructors.html#temporary-scopes
[`Vec::pop`]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.pop
[`String::len`]: https://doc.rust-lang.org/std/string/struct.String.html#method.len
[mrustc]: https://www.reddit.com/r/rust/comments/168qvrt/mrustc_now_has_half_of_a_borrow_checker/
[lint]: https://rust-lang.github.io/rust-clippy/master/index.html#/significant_drop_in_scrutinee

```rust
LINK: Playground ## playground://async_playground/tasks_no_join.rs
HIGHLIGHT: 8-17
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

    // Otherwise handle WAKERS and sleep as in Part One...
    ...
```

With all that in place, instead of hardcoding all whole task list in `main`, we
can define an `async_main` function and let it do the spawning:

```rust
LINK: Playground ## playground://async_playground/tasks_no_join.rs
HIGHLIGHT: 1-6,11
async fn async_main() {
    // The main loop currently waits for all tasks to finish.
    for n in 1..=10 {
        spawn(foo(n));
    }
}

fn main() {
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    let mut tasks: Vec<DynFuture> = vec![Box::pin(async_main())];
    ...
```

It works! Because of how we push and pop `NEW_TASKS`, the order of prints is
different now. We could fix that, but let's keep it the way it is. It's a good
reminder that, like threads, tasks running at the same time can run in any
order.

## JoinHandle

As we noted above, Tokio supports tasks that run in the background without
blocking program exit, and it also supports tasks with return
values.[^listen_loop] Both of those things require [`tokio::task::spawn`] to
return a [`tokio::task::JoinHandle`], very similar to how
[`std::thread::spawn`] returns a [`std::thread::JoinHandle`]. We'll implement
our own `JoinHandle` to get the same features. Also, the only way for our tasks
to block so far has been `sleep`, and introducing a second form of blocking
will lead to an interesting bug.

[^listen_loop]: We won't need task return values ourselves, but once we
    implement blocking, we'll see that carrying a value doesn't add any extra
    lines of code. We'll need non-blocking background tasks when we get to IO,
    though, so that our example can exit after "client" tasks are finished,
    without taking extra steps to shut down the "server" task.

[`tokio::task::spawn`]: https://docs.rs/tokio/latest/tokio/task/fn.spawn.html
[`tokio::task::JoinHandle`]: https://docs.rs/tokio/latest/tokio/task/struct.JoinHandle.html
[`std::thread::spawn`]: https://doc.rust-lang.org/std/thread/fn.spawn.html
[`std::thread::JoinHandle`]: https://doc.rust-lang.org/std/thread/struct.JoinHandle.html

`JoinHandle` needs to communicate between a task that's finishing and another
task that's waiting for it to finish. The waiting side needs somewhere to put
its `Waker` so that the finishing side can invoke it,[^one_waker] and the
finishing side needs somewhere to put its return value, `T`, so that the
waiting side can receive it. We won't need both of those things at the same
time, so we can use an `enum`. We need this `enum` to be shared and mutable, so
we'll wrap it in an `Arc`[^arc] and a `Mutex`:[^rc_refcell]

[^one_waker]: Note that we only need space for one `Waker`. It's possible that
    different calls to `poll` could supply different `Waker`s, but the
    [contract of `Future::poll`][contract] is that "only the `Waker` from the
    `Context` passed to the most recent call should be scheduled to receive a
    wakeup."

[contract]: https://doc.rust-lang.org/std/future/trait.Future.html#tymethod.poll

[^arc]: [`Arc`] is an atomic reference-counted smart pointer, similar to
    [`std::shared_ptr`] in C++. It behaves like a shared reference, but it's
    not bound to the lifetime of any particular scope. `Arc` is the standard
    way to share objects that don't have a fixed scope (so you can't use a
    shared reference) but also aren't global (so you can't use a `static`).

[`Arc`]: https://doc.rust-lang.org/std/sync/struct.Arc.html
[`std::shared_ptr`]: https://en.cppreference.com/w/cpp/memory/shared_ptr

[^rc_refcell]: If we had used `thread_local!` instead of `static` to implement
    `NEW_TASKS` above, and avoided the `Send` requirements that came with that,
    then we could also use `Rc` and `RefCell` here instead of `Arc` and
    `Mutex`.

```rust
LINK: Playground ## playground://async_playground/tasks_noop_waker.rs
enum JoinState<T> {
    Unawaited,
    Awaited(Waker),
    Ready(T),
    Done,
}

struct JoinHandle<T> {
    state: Arc<Mutex<JoinState<T>>>,
}
```

Awaiting the `JoinHandle` is how we wait for a task to finish, so `JoinHandle`
needs to implement `Future`. One tricky detail to watch out for is that the
waiting thread wants to take ownership of the `T` from `JoinState::Ready(T)`,
but `Arc<Mutex<JoinState>>` will only give us a reference to the `JoinState`
itself, and Rust won't let us "leave a hole" behind that reference. Instead, we
need to replace the whole `JoinState` with [`mem::replace`]:[^mem_take]

[`mem::replace`]: https://doc.rust-lang.org/std/mem/fn.replace.html

[^mem_take]: It would be more convenient if we could use [`mem::take`] directly
    on `&mut T`, but that only works if `T` implements [`Default`], and we
    don't want our `spawn` function to require that. Another option is a
    library called [`replace_with`], which does let us "leave a hole" behind
    any `&mut T` temporarily, but it's [not entirely clear][soundness] whether
    that approach is sound.

[`mem::take`]: https://doc.rust-lang.org/std/mem/fn.take.html
[`Default`]: https://doc.rust-lang.org/std/default/trait.Default.html
[`replace_with`]: https://docs.rs/replace_with
[soundness]: https://github.com/rust-lang/rfcs/pull/1736#issuecomment-1311564676

```rust
LINK: Playground ## playground://async_playground/tasks_noop_waker.rs
impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<T> {
        let mut guard = self.state.lock().unwrap();
        // Use JoinState::Done as a placeholder, to take ownership of T.
        match mem::replace(&mut *guard, JoinState::Done) {
            JoinState::Ready(value) => Poll::Ready(value),
            JoinState::Unawaited | JoinState::Awaited(_) => {
                // Replace the previous Waker, if any.
                *guard = JoinState::Awaited(context.waker().clone());
                Poll::Pending
            }
            JoinState::Done => unreachable!("polled again after Ready"),
        }
    }
}
```

At the other end of the communication, futures passed to `spawn` don't know
anything about `JoinState`, so we need a wrapper function to handle their
return values and invoke the `Waker` if there is one:[^async_block]

[^async_block]: Rust also supports async blocks, written `async { ... }`, which
    act like async closures that take no arguments. If we didn't want this
    adapter to be a standalone function, we could move its body into `spawn`
    below and make it an async block.

```rust
LINK: Playground ## playground://async_playground/tasks_noop_waker.rs
async fn wrap_with_join_state<F: Future>(
    future: F,
    join_state: Arc<Mutex<JoinState<F::Output>>>,
) {
    let value = future.await;
    let mut guard = join_state.lock().unwrap();
    if let JoinState::Awaited(waker) = &*guard {
        waker.wake_by_ref();
    }
    *guard = JoinState::Ready(value)
}
```

Now we can biuld a `JoinState` and apply that wrapper in `spawn`, so that it
accepts any `Output` type and returns a `JoinHandle`:[^send_bound]

[^send_bound]: `T` will need to be `Send` and `'static` just like `F`.
    Concretely, the future returned by `wrap_with_join_state` needs to be
    coercible to `DynFuture`, which means the `JoinState<T>` that it contains
    needs to be `Send` and `'static`, which means T needs to be too. This time
    around I'll skip the "discovery" phase and just write the bounds correctly
    the first time.

```rust
LINK: Playground ## playground://async_playground/tasks_noop_waker.rs
fn spawn<F, T>(future: F) -> JoinHandle<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let join_state = Arc::new(Mutex::new(JoinState::Unawaited));
    let join_handle = JoinHandle {
        state: Arc::clone(&join_state),
    };
    let task = Box::pin(wrap_with_join_state(future, join_state));
    NEW_TASKS.lock().unwrap().push(task);
    join_handle
}
```

We can collect and `.await` those `JoinHandle`s in `async_main`,[^rely_on_pin]
similar to how we used Tokio tasks above:[^unwrap]

[^rely_on_pin]: Shhh, do you hear that? [What you do not hear][iocaine] is the
    sound of relying on `Pin` guarantees: This version of `async_main` borrows
    `task_handles` across an `.await` point, so the returned future contains
    both `task_handles` and pointers to it. That would be unsound if the future
    could move after it was polled and after the borrow was established. This
    is the first time we've relied on `Pin` like this in our own code. We'll do
    it again in Part Three, when our IO futures start holding references.

[^unwrap]: The Tokio version had an extra `.unwrap()` after `handle.await`,
    because Tokio catches panics and converts them to `Result`s, again similar
    to `std::thread`. If we wanted to do the same thing, then inside of
    `wrap_with_join_state` above we'd use [`FutureExt::catch_unwind`], the
    async-adapted version of [`std::panic::catch_unwind`]. `JoinHandle::Output`
    would be the corresponding `Result`.

[`FutureExt::catch_unwind`]: https://docs.rs/futures/latest/futures/future/trait.FutureExt.html#method.catch_unwind
[`std::panic::catch_unwind`]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html

```rust
LINK: Playground ## playground://async_playground/tasks_noop_waker.rs
async fn async_main() {
    let mut task_handles = Vec::new();
    for n in 1..=10 {
        task_handles.push(spawn(foo(n)));
    }
    for handle in task_handles {
        handle.await;
    }
}
```

Now that we can explicitly wait on a task, we don't need to wait for all of
them automatically, and we want our main loop to exit after the main task is
finished. Let's split the main task out from the `tasks` list, which we can
rename to `other_tasks`:[^eagle_eyed]

[^eagle_eyed]: Eagle-eyed readers might spot that we're no longer coercing
    `main_task` to a `DynFuture`. That means that `async_main` doesn't have to
    return `()`. We'll take advantage of that in the Part Three to return
    `io::Result<()>`. Technically `async_main` doesn't have to be `Send`
    anymore either, but we won't mess with that.

```rust
LINK: Playground ## playground://async_playground/tasks_noop_waker.rs
HIGHLIGHT: 4-15
fn main() {
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    let mut main_task = Box::pin(async_main());
    let mut other_tasks: Vec<DynFuture> = Vec::new();
    loop {
        // Poll the main task and exit immediately if it's done.
        if main_task.as_mut().poll(&mut context).is_ready() {
            return;
        }
        // Poll other tasks and remove any that are Ready.
        let is_pending = |task: &mut DynFuture| {
            task.as_mut().poll(&mut context).is_pending()
        };
        other_tasks.retain_mut(is_pending);
        // Handle NEW_TASKS and WAKERS...
```

Done! That was a lot of changes all at once, so you might want to open [the
code from the last section][last] side-by-side with [the code from this
section][this]. Fortunately, it all builds. It even&hellip;_almost_ works. Our program prints the
correct output, but then it panics:

[last]: playground://async_playground/tasks_no_join.rs
[this]: playground://async_playground/tasks_noop_waker.rs

[iocaine]: https://www.youtube.com/watch?v=rMz7JBRbmNo

```
LINK: Playground ## playground://async_playground/tasks_noop_waker.rs
HIGHLIGHT: 2-6
...
end 3
end 2
end 1
thread 'main' panicked at tasks_noop_waker.rs:137:50:
sleep forever?
```

This is the interesting bug we were looking forward to!

## Waker

The panic is coming from this line, which has been in our main loop since the
end of Part One:

```rust
LINK: Playground ## playground://async_playground/tasks_noop_waker.rs
HIGHLIGHT: 2
let mut wake_times = WAKE_TIMES.lock().unwrap();
let next_wake = wake_times.keys().next().expect("sleep forever?");
thread::sleep(next_wake.saturating_duration_since(Instant::now()));
```

The loop is about to `sleep`, so it asks for the next wake time, but the
`WAKE_TIMES` tree is empty. Previously we could assume that if any task
returned `Pending`, there must be at least one wake time registered, because
the only source of blocking was `Sleep`. But now we have a second source of
blocking: `JoinHandle`. If a `JoinHandle` is `Pending`, it could be because the
other task is sleeping and will register a wake time. However, it could also be
that the other task is about to return `Ready` as soon as we poll it, and we
just haven't polled it yet. This is sensitive to the _order_ of our tasks list.
If a task at the front is waiting on a task at the back, we might end up with
`Pending` tasks and yet no wakeups scheduled.

That's exactly what's happened to us. Our main task is probably blocking on the
first `JoinHandle`. The main loop wakes up, polls the main task, and that
handle is still `Pending`. Then it polls all the `other_tasks`. Each of them
prints an "end" message, signals its `JoinHandle`, and returns `Ready`. At that
point, we need to poll the main task again instead of trying to sleep. But how
can the loop know that? Do we need to hack in another `static` flag? No,
there's already a way to tell the main loop not to sleep: `Waker`.

We've been using [`futures::task::noop_waker`] to supply a dummy `Waker` since
Part One. When `Sleep` was the only source of blocking, there was nothing for
the `Waker` to communicate, and all we needed was a placeholder to satisfy the
compiler. But now things have changed. Our `wrap_with_join_state` function is
already invoking `Waker`s correctly when tasks finish, and we need those
`Waker`s to _do something_. How do we write our own `Waker`?

[`futures::task::noop_waker`]: https://docs.rs/futures/latest/futures/task/fn.noop_waker.html

`Waker` implements `From<Arc<W>>`, where `W` is anything with the [`Wake`]
trait, which requires a `wake` method.[^RawWaker] That method takes `self:
Arc<Self>`, which is a little funny,[^clone] but apart from that it can do
whatever we like. The simplest option is to build what's effectively an
`Arc<Mutex<bool>>`[^atomic] and to set it to `true` when any task has received
a wakeup.[^waker_per_task] That's not so different from a `static` flag, but in
theory other people's futures can invoke our `Waker` without needing to know
the private implementation details of our main loop. Here's our glorified
`bool`:

[^RawWaker]: There's also [a fancy `unsafe` way to build a `Waker`][from_raw]
    from something called a [`RawWaker`]. That's what Tokio does, and it's what
    we'd have to do if we were targeting a `no_std` environment without `Arc`.

[from_raw]: https://doc.rust-lang.org/std/task/struct.Waker.html#method.from_raw
[`RawWaker`]: https://doc.rust-lang.org/std/task/struct.RawWaker.html
[`Wake`]: https://doc.rust-lang.org/alloc/task/trait.Wake.html

[^clone]: `Arc` is there because `Waker` is `Clone`. It would be nice if we
    could address that more directly with a bound like `W: Wake + Clone` on the
    `From` impl, but that turns out not to work because of a requirement of
    `dyn Trait` objects called ["object safety"][object_safe].

[object_safe]: https://huonw.github.io/blog/2015/01/object-safety

[^atomic]: Using [`AtomicBool`] for this would be more efficient, but `Mutex`
    is more familiar and good enough. If you want a three hour deep dive on
    atomics, listen to ["atomic<> Weapons" by Herb Sutter][atomic_weapons].
    That talk is focused on C++, but Rust and C both stole their atomic memory
    models from C++ anyway.

[`AtomicBool`]: https://doc.rust-lang.org/std/sync/atomic/struct.AtomicBool.html
[atomic_weapons]: https://www.youtube.com/watch?v=A8eCGOqgvH4

[^waker_per_task]: We could also construct a unique `Waker` for each task and
    then only poll the tasks that received wakeups. We saw
    [`futures::future::JoinAll`][join_all] do something like this in Part One.
    We could get this "for free" by replacing our tasks `Vec` with a
    [`FuturesUnordered`].

[join_all]: https://docs.rs/futures/latest/futures/future/fn.join_all.html
[`FuturesUnordered`]: https://docs.rs/futures/latest/futures/stream/struct.FuturesUnordered.html

```rust
LINK: Playground ## playground://async_playground/tasks.rs
struct AwakeFlag(Mutex<bool>);

impl AwakeFlag {
    fn check_and_clear(&self) -> bool {
        let mut guard = self.0.lock().unwrap();
        let check = *guard;
        *guard = false;
        check
    }
}

impl Wake for AwakeFlag {
    fn wake(self: Arc<Self>) {
        *self.0.lock().unwrap() = true;
    }
}
```

We can create an `AwakeFlag` and make a `Waker` with it at the start of `main`:

```rust
LINK: Playground ## playground://async_playground/tasks.rs
HIGHLIGHT: 2-4
fn main() {
    let awake_flag = Arc::new(AwakeFlag(Mutex::new(false)));
    let waker = Waker::from(Arc::clone(&awake_flag));
    let mut context = Context::from_waker(&waker);
    ...
```

Now we can finally add the check in the main loop that started this whole
discussion:[^another_deadlock]

[^another_deadlock]: The reason I defined `.check_and_clear()` above, instead
    of acquiring a `MutexGuard` here, is that we can create another deadlock if
    we forget to drop that guard. The last thing our main loop does is invoke
    `Wakers`, and `AwakeFlag::wake` takes the same lock.

```rust
LINK: Playground ## playground://async_playground/tasks.rs
HIGHLIGHT: 11-15
// Collect new tasks, poll them, and keep the ones that are Pending.
loop {
    let Some(mut task) = NEW_TASKS.lock().unwrap().pop() else {
        break;
    };
    // It's important that NEW_TASKS isn't locked here.
    if task.as_mut().poll(&mut context).is_pending() {
        other_tasks.push(task);
    }
}
// Some tasks might wake other tasks. Re-poll if the AwakeFlag has been
// set. Polling futures that aren't ready yet is inefficient but allowed.
if awake_flag.check_and_clear() {
    continue;
}
// Otherwise handle WAKERS and sleep as in Part One...
```

It works!

So, did we really do all this work just to have another way to spell
`join_all`? Well, yes and no. They're very similar on the inside. But we're
about to move beyond sleeping and printing to look at real IO, and we're going
to find that `spawn` is exactly what we want when we're listening for network
connections. Onward!

---

<div class="prev-next-arrows">
    <div><a href="async_futures.html">← Part One: Futures</a></div>
    <div class="space"> </div><div>
    <a href="async_io.html"> Part Three: IO →</a>
</div>
