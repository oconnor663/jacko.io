# Safety and Soundness in Rust <p class="subtitle">5 March 2023</p>

Rust is designed around safety and soundness. Roughly speaking, safe code is
code that doesn't use the `unsafe` keyword,[^safe_meanings] and sound code is
code that can't cause memory corruption or other undefined
behavior.[^undefined_behavior] One of Rust's most important features is the
promise that all safe code is sound. But that promise can be broken when
`unsafe` code is involved, and `unsafe` code is almost always involved
somewhere. Data structures like `Vec` and `HashMap` have `unsafe` code in their
implementations, as does any function like `File::open` that talks to the OS.
This leads to a common question: **"If Rust can't guarantee that all safe code
is sound, how can it be a memory-safe language?"** It's hard to give a short
answer to that question, so this post is my attempt at a medium-length answer.

## The short answer

This version is dense and technical. You might want to take a quick look at it,
move on to the next section, and then come back for another look at the end.

> Rust has a list of [behaviors considered
> undefined](https://doc.rust-lang.org/reference/behavior-considered-undefined.html).[^formal_spec]
> A "sound" function is one that maintains the following invariant: any program
> that only calls sound functions and doesn't contain any other `unsafe` code,
> can't commit UB.[^self_referential] A function that doesn't use any `unsafe`
> code, either directly or indirectly, is guaranteed to be
> sound.[^soundness_holes] A function that doesn't use any `unsafe` code
> directly and only calls other sound functions, is also sound by definition.
> But functions and modules that use `unsafe` code directly could be unsound,
> and a transitive caller of an unsound function could also be unsound. Any
> unsoundness in the safe, public API of a module is a bug.[^module_soundness]

## The medium-length answer

Consider the following Rust function, `foo1`, which reads a byte out of a
static string:[^implicit_return]

```rust
static BYTES: &[u8] = b"hello world";

fn foo1(index: usize) -> u8 {
    BYTES[index]
}
```

Here's a C version of `foo1`:

```c
const char* BYTES = "hello world";

char foo1(size_t index) {
    if (index >= strlen(BYTES)) {
        fprintf(stderr, "index out of bounds\n");
        exit(1);
    }
    return BYTES[index];
}
```

Both versions of `foo1` bounds-check the value of `index` before they use it.
This check is automatic in the Rust version. Because of this check, we can't
make `foo1` commit UB just by giving it a large `index`. Instead, the only way
I can think of to make `foo1` commit UB is to [give it an _uninitialized_
`index`](https://godbolt.org/z/e5bbq95hx). In C, we'd probably think of the
resulting UB as "the caller's fault". In Rust, using an uninitialized argument
[won't compile in safe
code](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=e23c9b052892c7c3e2b8bf5cd9f5cd98),
and doing it with `unsafe` code is [already UB in the
caller](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=be72905a4c634a62298d4aca5cca6dc4),
before we even get to the body of `foo1`. Since the Rust version of `foo1` will
never cause UB without the caller committing UB first, `foo1` is **sound**.
Rust guarantees that functions like `foo1`, which don't use any `unsafe` code
either directly or indirectly, will always be sound.

Now consider a slightly different function, `foo2`, which doesn't do a bounds
check:

```rust
unsafe fn foo2(index: usize) -> u8 {
    *BYTES.as_ptr().add(index)
}
```

Here's a C version of `foo2`:

```c
char foo2(size_t index) {
    return *(BYTES + index);
}
```

Calling either version of `foo2` with an `index` that's too large will read
past the end of `BYTES`, which is UB. Note that the Rust version of `foo2` is
declared `unsafe` in its signature, so calling it outside of another `unsafe`
function or `unsafe` block [is a compiler
error](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=ad9e08dd2e82a7411549a959c3eecf6b).
Since we can't call `foo2` in safe code, we don't usually ask whether `foo2` is
sound or unsound; we just say that it's "unsafe".[^unsafe_and_sound]
Dereferencing raw pointers like this isn't allowed in safe Rust, so deleting
the `unsafe` keyword [is also a compiler
error](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=e032302c44ce33a78b8c189ef488fc50).

But if we move the `unsafe` keyword down a bit, we start to get into trouble.
This function compiles:

```rust
fn foo3(index: usize) -> u8 {
    unsafe {
        *BYTES.as_ptr().add(index)
    }
}
```

`foo3` is like `foo2`, except we've removed the `unsafe` keyword from the
declaration and replaced it with an `unsafe` block in the body. That means we
can [call `foo3` and commit UB from safe
code](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=2546a5a170867d564e26ca01edd03b80).
In other words, `foo3` is **unsound**.

We can get in deeper trouble by adding some indirection:

```rust
fn foo4(index: usize) -> u8 {
    foo3(index)
}
```

`foo4` is a thin wrapper around `foo3`, so `foo4` is [also
unsound](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=a6d4a020eecfd0f01f8252ed24c4a254).
But `foo4` doesn't contain any `unsafe` code of its own. Instead, the
unsoundness of `foo3` has "infected" `foo4`. This sort of thing is why we can't
make a strong guarantee that all safe code is sound.

However, there's a slightly weaker guarantee that we can make. `foo4` doesn't
contain any `unsafe` code of its own, so it can't be unsound all by itself.
There must be some `unsafe` code somewhere that's
responsible.[^weird_exceptions] In this case of course, it's `foo3` that's
broken. There are two different ways we could fix `foo3`: We could declare that
it's `unsafe` in its signature, like `foo2`, which would [make `foo4` a
compiler
error](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=62bc28bc732a2c861544ccdfd1b4854d).
Or we could make it do bounds checks, like `foo1`, which would make `foo4`
sound with no changes. If we got rid of the `unsafe` code in `foo3`, then one
way or another Rust would make us do bounds checks.

So the simple promise of "no UB in safe code" can be broken. The slightly
weaker guarantee above is harder to explain, but it's the more correct idea,
and it's arguably Rust's most fundamental principle: **A safe caller can't be
"at fault" for memory corruption or other UB.**

In this sense, wrapping `unsafe` Rust in a safe API is similar to wrapping C
code in a Python API, or in any other memory-safe language.[^google_jni]
Mistakes in Python aren't supposed to cause memory corruption, and if they do,
we usually consider that a bug in the C bindings. Writing and reviewing
bindings isn't easy, but most applications contain little or no binding code of
their own. Similarly, most Rust applications contain little or no `unsafe` code
of their own, and memory corruption is rare.

---

Discussion threads on
[r/rust](https://www.reddit.com/r/rust/comments/11j8k8d/safety_and_soundness_in_rust/?),
[Hacker News](https://news.ycombinator.com/item?id=35032915), and
[lobste.rs](https://lobste.rs/s/tpstyz/safety_soundness_rust).

[^safe_meanings]: What we mean by "safe" depends on context, which is partly
  what this post is about. Sometimes we even talk about safety and soundness
  interchangeably, but here I want to emphasize the differences between them.

[^undefined_behavior]: "Undefined behavior" (UB) has a [specific
  meaning](https://en.wikipedia.org/wiki/Undefined_behavior) in languages like
  C, C++, and Rust, which is different from "unspecified" or
  "implementation-defined" behavior.

[^formal_spec]: Rust doesn't yet have a formal specification, but [there's
  general agreement](https://blog.m-ou.se/rust-standard/) that it needs one,
  and there's at least one [serious ongoing
  effort](https://ferrous-systems.com/ferrocene/) to write one. Shortcuts like
  "do what C does" are complicated by known gaps in the C specification in
  [areas like "pointer
  provenance"](https://www.ralfj.de/blog/2020/12/14/provenance.html). There are
  [ongoing experiments](https://github.com/rust-lang/rust/issues/95228) around
  how to close those gaps, and the [Miri](https://github.com/rust-lang/miri)
  project is also trying to make sure that the formal rules for UB will be
  programmatically checkable.

[^self_referential]: This definition is self-referential; the soundness of a
  function depends on what other functions are considered sound. It's possible
  to come up with two functions where either one could be sound, but not both
  at the same time. Niko Matsakis described how [a hypothetical safe wrapper
  around
  `setjmp`/`longjmp`](http://smallcultfollowing.com/babysteps/blog/2016/10/02/observational-equivalence-and-unsafe-code/#composing-unsafe-abstractions)
  could be sound in combination with "fundamental" Rust but unsound in
  combination with common (and now
  [standard](https://doc.rust-lang.org/stable/std/thread/fn.scope.html))
  threading libraries. There are [a few other known
  examples](https://github.com/rust-lang/unsafe-code-guidelines/issues/379) of
  "soundness forks", but these issues are rare in application code.

[^soundness_holes]: The Rust compiler has [known
  bugs](https://lcnr.de/blog/diving-deep-implied-bounds-and-variance/) where it
  accepts some programs that should've failed to compile, and these bugs make
  it possible for 100% safe programs to commit UB. We call these bugs
  "soundness holes". It's rare for these to affect real-world code, though, and
  the minimized examples that trigger them are often pretty hard to understand.
  All the soundness holes we know of will get fixed eventually. Formally
  proving that Rust can fix all its soundness holes is a [major research
  project](https://plv.mpi-sws.org/rustbelt/) and the sort of thing you might
  [write your PhD thesis about](https://research.ralfj.de/thesis.html).

[^module_soundness]: We usually evaluate soundness at module boundaries,
  because a safe write to a private field that other `unsafe` code depends on
  is often enough to commit UB. For example, any function in the implementation
  of `Vec` could overwrite the private `len` field and then do out-of-bounds
  reads and writes without using the `unsafe` keyword directly.

[^implicit_return]: When the last line of a Rust function doesn't end in a
  semicolon, that's an implicit `return`.

[^unsafe_and_sound]: In theory there's nothing wrong with a function that's
  both sound and `unsafe`, but in practice it's odd. Why not allow safe code to
  call the function, if it can't lead to UB? One answer could be that the
  function is expected to become unsound in the future, so it's marked `unsafe`
  now for compatibility.

[^google_jni]: The Google Security Blog [made a similar
  comparison](https://security.googleblog.com/2022/12/memory-safe-languages-in-android-13.html)
  between `unsafe` Rust and JNI in Java.

[^weird_exceptions]: Apart from "soundness holes" in the compiler, it's also
  possible for safe code to corrupt memory by asking the OS to do it in ways
  the compiler doesn't know about. This includes tricks like writing to
  `/proc/$PID/mem`, or spawning a debugger and attaching it to yourself. If we
  wanted to execute _malicious_ safe code and still guarantee memory safety,
  we'd need lots of help from the OS, and relying on process isolation instead
  of memory safety would probably make more sense.
