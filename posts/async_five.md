# Async Rust, Part Five: More!
###### \[date]

- [Part One: Why?](async_intro.html)
- [Part Two: Futures](async_futures.html)
- [Part Three: Tasks](async_tasks.html)
- [Part Four: IO](async_io.html)
- Part Five: More! (you are here)

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
