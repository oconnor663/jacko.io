# Safety and Soundness in Rust
###### 2023 March 5<sup>th</sup>

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
> that only calls sound functions, and doesn't contain any other `unsafe` code,
> can't commit UB.[^self_referential] Functions that don't use any `unsafe`
> code, either directly or indirectly, are guaranteed to be
> sound.[^soundness_holes] Functions that don't use any `unsafe` code
> directly, and only call other sound functions, are also sound by definition.
> But functions and modules that use `unsafe` code directly could be unsound,
> and callers of an unsound function could also be unsound. Any unsoundness in
> the safe, public API of a module is a bug.[^module_soundness]

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
`index`][uninitialized_index]. In C, we'd probably think of the resulting UB as
"the caller's fault". In Rust, using an uninitialized argument [won't compile
in safe code][safe_code], and doing it with `unsafe` code is [already UB in the
caller][in_the_caller], before we even get to the body of `foo1`. Since the
Rust version of `foo1` will never cause UB without the caller writing `unsafe`
first,[^fclose] `foo1` is **sound**. Rust guarantees that functions like
`foo1`, which don't use any `unsafe` code either directly or indirectly, will
always be sound.

[uninitialized_index]: <https://godbolt.org/#g:!((g:!((g:!((h:codeEditor,i:(filename:'1',fontScale:14,fontUsePx:'0',j:1,lang:___c,selection:(endColumn:2,endLineNumber:19,positionColumn:2,positionLineNumber:19,selectionStartColumn:2,selectionStartLineNumber:19,startColumn:2,startLineNumber:19),source:'%23include+%3Cstddef.h%3E%0A%23include+%3Cstdio.h%3E%0A%23include+%3Cstdlib.h%3E%0A%23include+%3Cstring.h%3E%0A%0Aconst+char*+BYTES+%3D+%22hello+world%22%3B%0A%0Achar+foo1(size_t+index)+%7B%0A++++if+(index+%3E%3D+strlen(BYTES))+%7B%0A++++++++fprintf(stderr,+%22index+out+of+bounds%5Cn%22)%3B%0A++++++++exit(1)%3B%0A++++%7D%0A++++return+BYTES%5Bindex%5D%3B%0A%7D%0A%0Aint+main()+%7B%0A++++size_t+index%3B+//+uninitialized!!%0A++++foo1(index)%3B%0A%7D'),l:'5',n:'0',o:'C+source+%231',t:'0')),k:50,l:'4',n:'0',o:'',s:0,t:'0'),(g:!((h:executor,i:(argsPanelShown:'1',compilationPanelShown:'0',compiler:cclang1500,compilerName:'',compilerOutShown:'0',execArgs:'',execStdin:'',fontScale:14,fontUsePx:'0',j:1,lang:___c,libs:!(),options:'-fsanitize%3Dmemory',overrides:!(),runtimeTools:!(),source:1,stdinPanelShown:'1',tree:'1',wrap:'1'),l:'5',n:'0',o:'Executor+x86-64+clang+15.0.0+(C,+Editor+%231)',t:'0')),k:50,l:'4',n:'0',o:'',s:0,t:'0')),l:'2',n:'0',o:'',t:'0')),version:4>

[safe_code]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=static+BYTES%3A+%26%5Bu8%5D+%3D+b%22hello+world%22%3B%0A%0Afn+foo1%28index%3A+usize%29+-%3E+u8+%7B%0A++++BYTES%5Bindex%5D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+index%3B+%2F%2F+uninitialized%0A++++foo1%28index%29%3B%0A%7D%0A

[in_the_caller]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Amem%3A%3AMaybeUninit%3B%0Ause+std%3A%3Aptr%3A%3Acopy_nonoverlapping%3B%0A%0Afn+foo1%28_index%3A+usize%29+%7B%0A++++%2F%2F+The+body+here+doesn%27t+matter.+In+this+example+it%27s+empty.%0A%7D%0A%0Afn+main%28%29+%7B%0A++++%2F%2F+Note+that+we+*don%27t*+use+mem%3A%3Aunitialized%28%29+or+MaybeUninit%3A%3Aassume_init%28%29%0A++++%2F%2F+here%2C+because+we+want+to+demonstrate+UB+on+line+22+below%2C+and+either+of%0A++++%2F%2F+those+approaches+would+actually+be+UB+right+here%2C+because+returning+an%0A++++%2F%2F+uninitialized+integer+is+considered+%22using%22+it+%28today%29.%0A++++let+mut+index+%3D+0%3B%0A%0A++++%2F%2F+Copy+an+uninitialized+value+into+%60index%60.+Because+%60copy_nonoverlapping%60%0A++++%2F%2F+is+allowed+to+handle+uninitialized+values%2C+this+isn%27t+per+se+UB+%28today%29.%0A++++%2F%2F+The+C+equivalent+of+%60copy_nonoverlapping%60+is+%60memcpy%60.%0A++++unsafe+%7B%0A++++++++copy_nonoverlapping%28MaybeUninit%3A%3Auninit%28%29.as_ptr%28%29%2C+%26mut+index%2C+1%29%3B%0A++++%7D%0A%0A++++%2F%2F+Even+though+the+body+of+%60foo1%60+is+empty%2C+calling+it+with+an+uninitialized%0A++++%2F%2F+argument+is+UB+%28today%29.+If+you+run+this+with+Tools-%3EMiri+above%2C+it+fails%0A++++%2F%2F+on+this+line.%0A++++foo1%28index%29%3B%0A%7D%0A

Now consider a slightly different function, `foo2`, which doesn't do a bounds
check:[^unsafe_block_in_unsafe_function]

[^unsafe_block_in_unsafe_function]: Rust originally and currently treats
    `unsafe` functions as though their entire body is wrapped in an `unsafe`
    block. Prior to Rust 1.65, there was even a warning for "unnecessary"
    `unsafe` blocks in `unsafe` functions. However, minimizing the scope of
    `unsafe` blocks has always been recommended, and eventually folks decided
    that explicit `unsafe` blocks in `unsafe` functions would be better. As of
    the 2024 Edition, `foo2` [generates a warning].

[generates a warning]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&code=static+BYTES%3A+%26%5Bu8%5D+%3D+b%22hello+world%22%3B%0A%0Apub+unsafe+fn+foo2%28index%3A+usize%29+-%3E+u8+%7B%0A++++*BYTES.as_ptr%28%29.add%28index%29%0A%7D

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
function or `unsafe` block [is a compiler error]. Since we can't call `foo2` in
safe code, we don't usually ask whether it's sound or unsound; we just say that
it's "unsafe".[^unsafe_and_sound] Dereferencing raw pointers like this isn't
allowed in safe Rust, so deleting the `unsafe` keyword [is also a compiler
error].

[is a compiler error]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=static+BYTES%3A+%26%5Bu8%5D+%3D+b%22hello+world%22%3B%0A%0Aunsafe+fn+foo2%28index%3A+usize%29+-%3E+u8+%7B%0A++++*BYTES.as_ptr%28%29.add%28index%29%0A%7D%0A%0Afn+main%28%29+%7B%0A++++foo2%280%29%3B%0A%7D%0A

[is also a compiler error]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=static+BYTES%3A+%26%5Bu8%5D+%3D+b%22hello+world%22%3B%0A%0Afn+foo2%28index%3A+usize%29+-%3E+u8+%7B%0A++++*BYTES.as_ptr%28%29.wrapping_add%28index%29%0A%7D

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
can [call `foo3` and commit UB from safe code][commit_ub]. In other words,
`foo3` is **unsound**.

[commit_ub]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=static+BYTES%3A+%26%5Bu8%5D+%3D+b%22hello+world%22%3B%0A%0Afn+foo3%28index%3A+usize%29+-%3E+u8+%7B%0A++++unsafe+%7B%0A++++++++*BYTES.as_ptr%28%29.add%28index%29%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++%2F%2F+Use+an+index+that%27s+way+too+large.+In+debug+mode%2C+this+will+segfault.+If%0A++++%2F%2F+you+run+it+with+Tools+-%3E+Miri+in+the+top+right%2C+Miri+will+print+an%0A++++%2F%2F+%22out-of-bounds+pointer+arithmetic%22+error.%0A++++foo3%280xffffffff%29%3B%0A%7D%0A

We can get in deeper trouble by adding some indirection:

```rust
fn foo4(index: usize) -> u8 {
    foo3(index)
}
```

`foo4` is a thin wrapper around `foo3`, so `foo4` is [also
unsound].
But `foo4` doesn't contain any `unsafe` code of its own. Instead, the
unsoundness of `foo3` has "infected" `foo4`. This sort of thing is why we can't
make a strong guarantee that all safe code is sound.

[also unsound]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=static+BYTES%3A+%26%5Bu8%5D+%3D+b%22hello+world%22%3B%0A%0Afn+foo3%28index%3A+usize%29+-%3E+u8+%7B%0A++++unsafe+%7B%0A++++++++*BYTES.as_ptr%28%29.add%28index%29%0A++++%7D%0A%7D%0A%0Afn+foo4%28index%3A+usize%29+-%3E+u8+%7B%0A++++foo3%28index%29%0A%7D%0A%0Afn+main%28%29+%7B%0A++++%2F%2F+Use+an+index+that%27s+way+too+large.+In+debug+mode%2C+this+will+segfault.+If%0A++++%2F%2F+you+run+it+with+Tools+-%3E+Miri+in+the+top+right%2C+Miri+will+print+an%0A++++%2F%2F+%22out-of-bounds+pointer+arithmetic%22+error.%0A++++foo4%280xffffffff%29%3B%0A%7D%0A

However, there's a slightly weaker guarantee that we can make. `foo4` doesn't
contain any `unsafe` code of its own, so it can't be unsound all by itself.
There must be some `unsafe` code somewhere that's
responsible.[^weird_exceptions] In this case of course, it's `foo3` that's
broken. There are two different ways we could fix `foo3`: We could declare that
it's `unsafe` in its signature like `foo2`, which would [make `foo4` a compiler
error][compiler_error]. Or we could make it do bounds checks like `foo1`, which
would make `foo4` sound with no changes. If we got rid of the `unsafe` code in
`foo3`, then one way or another Rust would make us do bounds checks.

[compiler_error]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=static+BYTES%3A+%26%5Bu8%5D+%3D+b%22hello+world%22%3B%0A%0Aunsafe+fn+foo3%28index%3A+usize%29+-%3E+u8+%7B%0A++++*BYTES.as_ptr%28%29.add%28index%29%0A%7D%0A%0Afn+foo4%28index%3A+usize%29+-%3E+u8+%7B%0A++++foo3%28index%29%0A%7D

So the simple promise of "no UB in safe code" can be broken. The slightly
weaker guarantee above is harder to explain, but it's the more correct idea,
and it's arguably Rust's most fundamental principle: **A safe caller can't be
"at fault" for memory corruption or other UB.**

In this sense, wrapping `unsafe` Rust in a safe API is like wrapping C
in a Python API, or in any other memory-safe language.[^google_jni]
Mistakes in Python aren't supposed to corrupt memory, and if they do,
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
  bugs](https://github.com/rust-lang/rust/issues/25860) where it
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

[^fclose]: This line originally read "without the caller committing UB first",
  but Peter Ammon [pointed out](https://news.ycombinator.com/item?id=35035347)
  that printing to `stderr` can become UB after `fclose(stderr)` or `fork()`.

[^unsafe_and_sound]: In theory there's nothing wrong with a function that's
  both sound and `unsafe`, but in practice it's odd. Why not allow safe code to
  call the function, if it can't lead to UB? One answer could be that the
  function is expected to become unsound in the future, so it's marked `unsafe`
  now for compatibility.

[^weird_exceptions]: Apart from "soundness holes" in the compiler, it's also
  possible for safe code to corrupt memory by asking the OS to do it in ways
  the compiler doesn't know about. This includes tricks like writing to
  `/proc/$PID/mem`, or spawning a debugger and attaching it to yourself. If we
  wanted to execute _malicious_ safe code and still guarantee memory safety,
  we'd need lots of help from the OS, and relying on process isolation instead
  of memory safety would probably make more sense.

[^google_jni]: The Google Security Blog [made a similar
  comparison](https://security.googleblog.com/2022/12/memory-safe-languages-in-android-13.html)
  between `unsafe` Rust and JNI in Java.
