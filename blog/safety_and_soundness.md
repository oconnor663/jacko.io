> This is a draft. I'd like this article to be as accurate as reasonably
> possible (for its length), and I want to get feedback and corrections before
> I publish it anywhere more broadly.

# Safety and Soundness in Rust

Rust is designed around **safety** and **soundness**. Roughly speaking, safe
code is code that doesn't use the `unsafe` keyword,[^safe_meanings] and sound
code is code that can't cause memory corruption or other undefined behavior
(UB).[^undefined_behavior] One of Rust's most important features is the promise
that all safe code is sound. But that promise can be broken when `unsafe` code
is involved, and `unsafe` code is almost always involved somewhere. Standard
data structures like `Vec` and `HashMap` have `unsafe` code in their
implementations, as does any function like `File::open` that talks to the OS.
This leads to a common question: **"If Rust can't actually guarantee that all
safe code is sound, how is it any safer than C or C++?"** It's hard to give a
short answer to that question, so this article is my attempt at a medium-length
answer.

## Ok but actually, what's the short answer?

I don't like how dense and abstract this is, but I've tried as best I can to
make it correct. Feel free to skip this part.

Rust has a list of [behaviors considered
undefined](https://doc.rust-lang.org/reference/behavior-considered-undefined.html).[^formal_spec]
We define "sound" functions like this: any program that only calls sound
functions, and doesn't contain any other `unsafe` code, can't commit
UB.[^self_referential] A function that doesn't use any `unsafe` code, either
directly or indirectly, is guaranteed to be sound.[^soundness_holes] A function
that doesn't use any `unsafe` code directly, and only calls other sound
functions, is also sound by definition. But functions and modules that use
`unsafe` code directly have to be careful not to commit UB, and also not to
allow their safe callers to commit UB. Any unsoundness in the safe, public API
of a module is a bug.[^module_soundness] There's no formal guarantee that the
set of sound functions will be *useful*, but in practice it is, and most
applications contain little or no `unsafe` code.

Let's build up to this with examples.

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
This check is automatic in the Rust version, but in C we need to write it
ourselves. Because of this check, we can't make `foo1` commit UB just by giving
it a large `index`. Instead, the only way I can think of to make `foo1` commit
UB is to [give it an _uninitialized_
`index`](https://godbolt.org/z/e5bbq95hx).[^uninitialized] In C, we'd probably
think of the resulting UB as "the caller's fault". In Rust, using an
uninitialized argument [won't compile in safe
code](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=e23c9b052892c7c3e2b8bf5cd9f5cd98),
and doing it with `unsafe` code is [already UB in the
caller](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=be72905a4c634a62298d4aca5cca6dc4),
before we even get to the body of `foo1`. Since the Rust version of `foo1` will
never cause UB without the caller committing UB first, `foo1` is *sound*. Rust
guarantees that functions like `foo1`, which don't use any `unsafe` code either
directly or indirectly, will always be sound.

Now consider a slightly different function, `foo2`, which doesn't do a bounds
check:

```rust
unsafe fn foo2(index: usize) -> u8 {
    *BYTES.as_ptr().add(index)
}
```

Here's the C version of `foo2`:

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
Since we can't call `foo2` in safe code, we don't usually ask whether it's
sound or unsound; we just call it "unsafe".[^unsafe_and_sound] Dereferencing
raw pointers like this isn't allowed in safe Rust, so deleting the `unsafe`
keyword [is also a compiler
error](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=e032302c44ce33a78b8c189ef488fc50).

But things start to go wrong if we move the `unsafe` keyword down a bit. This
function compiles:

```rust
fn foo3(index: usize) -> u8 {
    unsafe {
        *BYTES.as_ptr().add(index)
    }
}
```

`foo3` is like `foo2`, except that we've removed the `unsafe` keyword from the
declaration and replaced it with an `unsafe` block in the body. That means we
can [call `foo3` and commit UB from safe
code](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=2546a5a170867d564e26ca01edd03b80).
In other words, `foo3` is *unsound*.

We can make the problem worse by adding some indirection:

```rust
fn foo4(index: usize) -> u8 {
    foo3(index)
}
```

`foo4` is just a thin wrapper around `foo3`, so `foo4` is [also
unsound](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=a6d4a020eecfd0f01f8252ed24c4a254).
But `foo4` doesn't contain any `unsafe` code of its own. Instead, the
unsoundness of `foo3` has "infected" `foo4`. This is why we can't make a strong
guarantee that all safe code is sound.

However, there's a slightly weaker guarantee that we _can_ make. `foo4` doesn't
contain any `unsafe` code, so it can't be unsound all by itself. There must be
some `unsafe` code somewhere else that's *responsible*.[^weird_exceptions] In
this case of course, it's `foo3` that's broken. There are two different ways we
could fix `foo3`: We could declare that it's `unsafe` in its signature, like
`foo2`, which would [make `foo4` a compiler
error](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=62bc28bc732a2c861544ccdfd1b4854d).
Or we could make it do bounds checks, like `foo1`, which would make `foo4`
sound with no changes. If we got rid of the `unsafe` code in `foo3`, then one
way or another Rust would make us do bounds checks.

So the simple promise of "no UB in safe code" can be broken. The slightly
weaker guarantee above is harder to explain, but it's the more correct idea,
and it's arguably Rust's most fundamental principle: **A safe caller can't be
"at fault" for memory corruption or other UB.**

This relationship between safe Rust and `unsafe` Rust is similar to what we
expect when we use a memory-safe language like Python or Java to call into
"native" C libraries.[^google_jni] If the result is memory corruption, we often
consider that a bug in the native bindings. Our application code could be buggy
too, but the language is supposed to handle that by throwing safe exceptions,
and we usually expect our bindings to do whatever wrapping or checking they
need to do to make that safe. This is a high bar for correctness, but it's also
a clear contract that authors and reviers can verify locally. Python and Java
applications call into native libraries all the time, but most contain little
or no binding code of their own, and memory corruption is rare.

## The catch

The catch is that Rust has strict lifetime and aliasing rules. For example,
Rust [won't usually let
us](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=5f2b90835dfe746b18d1bddaa43275f5)
increment an integer while we hold a pointer to it (other than [through that
pointer](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=83005426f1587180f47aba95e05843e0)).
Similarly, Rust [won't usually let
us](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=2c84a02b7a9bf48b45543d23201495cd)
create a reference cycle between two objects. These rules take a lot of getting
used to, and using `unsafe` code to break them is [tempting but usually
UB](https://youtu.be/DG-VLezRkYQ).

Here are some things that are difficult, slow, and/or impossible to do without
`unsafe` code:

- call C functions without existing bindings
- read and write C-style unions
- implement cyclic data structures like doubly-linked lists and graphs[^linked_lists]
- write maximum-performance SIMD code or raw assembly
- memory-map a file

And for completeness, here are some usually-buggy behaviors that aren't
considered UB. These can all happen in safe code:

- deadlocks
- memory leaks[^memory_leaks]
- race conditions that aren't "data races"[^data_races]
- arithmetic overflows[^overflows]
- assertion failures and other aborts

[^safe_meanings]: The `unsafe` keyword can show up in different places: in the
  signature of a function, in its body, in the body of some other function it
  calls, or in [`unsafe` trait
  implementations](https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html#implementing-an-unsafe-trait).
  Each of those has different consequences, so what we mean when we say that
  some piece of code is "safe" depends on context. Sometimes we also talk about
  safety and soundness interchangeably. But in this article I want to emphasize
  and clarify the differences between them.

[^undefined_behavior]: "Undefined behavior" (UB) has a specific meaning in
  languages like C, C++, and Rust, which might not be familiar to folks coming
  from languages like Python or Java. UB is different from "unspecified" or
  "undocumented" behavior. It comes up most often when we work with things like
  pointers or uninitialized memory, where breaking the rules means our program
  could do almost anything, including running arbitrary code from some
  attacker. This is a common source of security vulnerabilities.

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

[^module_soundness]: A private function in a module might be able to commit UB
  without `unsafe` code, by modifying a private field that other `unsafe` code
  relies on. For example, any function in the implementation of `Vec` could
  overwrite the private `len` field and then do out-of-bounds reads and writes
  without using the `unsafe` keyword directly. This can lead to unsoundness in
  safe-looking private helper functions. But whether such functions should
  always be marked `unsafe` is a matter of taste, as long as the module's
  public API is sound.

[^implicit_return]: When the last line of a Rust function doesn't end in a
  semicolon, that's an implicit `return`. Similarly, if the last line of a code
  block doesn't end in a semicolon, it's the value of the block.

[^uninitialized]: An "uninitialized" variable is one that's been given a name
  but no value. This doesn't come up in langauges like Python, which requires
  an initial value when you declare a variable, or in languages like Go, where
  variables without an initial value get a default zero/empty value. But it
  does come up in C, C++, and `unsafe` Rust, and it can cause confusing UB. For
  example, an uninitialized variable might not have any consistent value at
  all, which can lead to seemingly impossible outcomes like passing a bounds
  check and then indexing out of bounds.

[^unsafe_and_sound]: In theory there's nothing wrong with a function that's
  both sound and `unsafe`, but in practice it's odd. Why not allow safe code to
  call the function, if it can't lead to UB? One answer could be that the
  function is expected to become unsound in the future, so it's marked `unsafe`
  now for compatibility.

[^google_jni]: I'm lifting this analogy from Google Security Blog post about
  [Memory Safe Languages in Android
  13](https://security.googleblog.com/2022/12/memory-safe-languages-in-android-13.html).

[^weird_exceptions]: Apart from "soundness holes"[^soundness_holes] in the
  compiler, it's also possible to corrupt memory by asking the OS to do it for
  you in ways the compiler doesn't know about. This includes tricks like
  writing to `/proc/$PID/mem`, or spawning a debugger and attaching it to
  yourself. If we wanted to execute _malicious_ safe code and still guarantee
  memory safety, we'd need lots of help from the OS, and relying on process
  isolation instead of memory safety would probably make more sense.

[^linked_lists]: Implementing new data structures is relatively more
  complicated in Rust than in other languages, because of the lifetime and
  aliasing rules. This is especially true of pointer-based data structures like
  linked lists and trees, which tend to require `Option<Box<T>>` and other
  tricky patterns, and _especially_ especially true of cyclic data structures
  like doubly-linked lists and graphs, which tend to require `unsafe` code.
  This can be surprising to programmers coming from other languages, where
  implementing a linked list is a beginner project. For an exhaustive study of
  these issues, see [Learn Rust With Entirely Too Many Linked
  Lists](https://rust-unofficial.github.io/too-many-lists/)

[^memory_leaks]: Rust usually frees memory automatically in destructors, so
  memory leaks are rare in practice, but it's possible to prevent destructors
  from running in safe code. You can do this deliberately by calling
  [`std::mem::forget`](https://doc.rust-lang.org/std/mem/fn.forget.html) or
  creating a
  [`ManuallyDrop`](https://doc.rust-lang.org/std/mem/struct.ManuallyDrop.html)
  object. The most common way to do this accidentally is to create a reference
  cycle with [`Rc`](https://doc.rust-lang.org/std/rc/struct.Rc.html) or
  [`Arc`](https://doc.rust-lang.org/std/sync/struct.Arc.html), the
  reference-counted smart pointer types, which are similar to
  [`std::shared_ptr`](https://en.cppreference.com/w/cpp/memory/shared_ptr) in
  C++.

[^data_races]: A "data race" is a specific kind of race condition, where one
  thread is writing something in memory while another thread is reading or
  writing the same thing, without locks, atomics, or some other
  synchronization. This is always UB in C, C++, and Rust, even in cases where
  the underlying hardware might be ok with it. On the other hand, an example of
  a race condition that isn't a data race could be two threads printing at the
  same time, where the order of their prints might change from run to run.

[^overflows]: Arithmetic overflow is defined behavior in Rust. By default it
  panics in debug mode and wraps in release mode, but this is
  [configurable](https://doc.rust-lang.org/cargo/reference/profiles.html#overflow-checks).
  Rust integer types also support explicit methods like
  [`wrapping_add`](https://doc.rust-lang.org/stable/std/primitive.i32.html#method.wrapping_add)
  and
  [`checked_add`](https://doc.rust-lang.org/stable/std/primitive.i32.html#method.checked_add),
  for cases where overflow is expected.
