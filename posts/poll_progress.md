`poll_progress` on an `async gen fn` calls `poll_progress` on any
`AsyncIterator`s that its body is currently in a `for await` loop over (either
suspended at an `.await` or at a `yield` in the loop body, though not while the
`AsyncIterator` itself is pending). It does *not* advance control through
`async gen fn` body itself. Only `poll_next` does that. The common default is
that streams and their consumers run sequentially, not concurrently. This is
important to callers who don't want to over-consume items if their `for await`
body short-circuits, and more generally so that "async code that looks like
regular code behaves like regular code".

As a result, in a program that only uses `async gen fn` and no buffering
helpers, `poll_progress` basically has no effect. However, in a hypothetical
future example like this...

```rs
async gen fn foo() {
    join {
        // `foo::poll_progress` drives this half of the `join` (in the spirit
        // of `poll_next`) if we last yielded from the other half, until it
        // reaches its own `yield`. This side also might not have a `yield` (in
        // which case `poll_progress` drives it more in the spirit of `poll`).
        while random() {
            sleep(random()).await;
            yield 42;
        }
    } and {
        for item in bar() {
            // `foo::poll_progress` calls `bar::poll_progress` when this half
            // of the `join` is suspended at this `.await` or this `yield`.
            do_something_with(item).await;
            yield 43;
        }
    }
}
```

...`poll_progress` would drive whichever half lost the `yield` race. That means
the `foo` body's `AsyncIterator` impl would need space to store at least one
yielded item.

It is only valid to call `poll_progress` when the body is suspended at a
`yield`.
