# Async Rust, Part Three: More!
###### \[date]

- [Part One: Why?](async_one.html)
- [Part Two: How?](async_two.html)
- Part Three: More! (you are here)

## Cancellation

[`timeout()` example][timeout]

[timeout]: playground://async_playground/timeout.rs

## Recursion

Regular recursion doesn't work:

```rust
LINK: Playground playground://async_playground/compiler_errors/recursion.rs
async fn factorial(n: u64) -> u64 {
    if n == 0 {
        1
    } else {
        n * factorial(n - 1).await
    }
}
```

We need to box the thing:


```rust
LINK: Playground playground://async_playground/boxed_recursion.rs
async fn factorial(n: u64) -> u64 {
    if n == 0 {
        1
    } else {
        let recurse = Box::pin(factorial(n - 1));
        n * recurse.await
    }
}
```

## Tasks

[Tokio tasks example][tokio_tasks]

[tokio_tasks]: playground://async_playground/tokio_tasks.rs

[Here's a custom event loop with a growable list of tasks.][custom_tasks]

[custom_tasks]: playground://async_playground/tasks.rs

## Pin

[an example of writing our own `Future` trait without `Pin`][no_pin]

[no_pin]: playground://async_playground/no_pin.rs

## IO
