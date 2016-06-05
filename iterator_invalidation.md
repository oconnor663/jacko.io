# Iterator invalidation in Rust

## or: ARGH why can't this just work like it does in Python?

When I code in Rust, the borrow checker yells at me a lot. On a good day, I
need to reorder some variables or stick in a pair of curly braces. On a bad
day, what I've written is deeply offensive to the compiler, and nothing I do is
going to make it ok. That's annoying when I know my program would be totally
fine in a different langauge. And it's particularly annoying when all I'm doing
is looping over a list.

Here's some code that Python is perfectly willing to run:

```python
mylist = [1, 2, 3]
for i in mylist:
    if i == 2:
        mylist.append(4)
```

Sure, I can modify a list while I'm iterating over it. It might feel a little
dirty, but anyway the meaning is clear. "When you get to 2, append the 4, then
keep on going." Simple.

Rust disagrees.

```rust
let mut mylist = vec![1, 2, 3];
for i in &mylist {
    if *i == 2 {
        mylist.push(4); // ERROR: cannot borrow `mylist` as mutable
    }                   // because it is also borrowed as immutable
}

```

Aww c'mon Rust! Why does this have to be so hard? I know it's "against the
rules" for anything to alias a mutable reference, but it feel like such an
arbitrary limitation right now. Why can't you just do what Python does? It's
not like this is going to cause *undefined behavior*...is it?

Yes. Yes it sure is.

[Python envelope] [Rust envelope]

The big difference between Python and Rust in these examples is the variable
`i`. In Python, `i` points to an integer that has a life of its own somewhere.
If `mylist` disappears, `i` will still be perfectly valid. In Rust however, `i`
points to an integer that lives *inside* of `mylist`'s memory. If Rust lets us
do `mylist.push(4)`, then `mylist` will need to grow, and its memory will move
around. That turns `i` into a dangling pointer! (C++ programmers reading along
are like "welcome to my life".)

Lists in Python don't share their memory with anything else. That makes it safe
to grow a list or free it, but it comes at a performance cost. Python needs to
allocate memory separately for each element of a list, instead of fitting
everything into one contiguous chunk. Python also needs to make copies of a
list's memory when you take a slice of it. Rust on the other hand can store
everything in one chunk, and let you have references and slices directly into
that memory, but then it has to be much more careful about what happens to the
vector while those references are still alive.

## Not quite the whole story

Python actually *does* have a way to slice memory without copying it. Check
this out:

```python
mybytes = bytearray(b"foobar")
myslice = memoryview(mybytes)[0:3]
mybytes[1:3] = b"ee"
print(myslice.tobytes())  # b'fee'
```

Through the magic of `memoryview`, `myslice` is really truly a slice of
`mybytes`. So how does Python deal with the moving memory problem?

```
>>> mybytes.extend(b"baz")
Traceback (most recent call last):
  File "<stdin>", line 1, in <module>
BufferError: Existing exports of data: object cannot be re-sized
```

`bytearray` increments a counter whenever you take a `memoryview` out of it. As
long as a view exists, the `bytearray` isn't allowed to resize. Python's usual
reference counting also guarantees that the `bytearray` won't be freed.

It's also possible to implement a Python-style list in Rust, though to make it
work you have to [reference count everything](https://is.gd/Bqh1dO).
