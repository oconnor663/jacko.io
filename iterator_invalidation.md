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

Yes it is. Yes it sure is.

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

`bytearray` increments a counter when you take a `memoryview` out of it. As
long as a view exists, the `bytearray` isn't allowed to resize. Python's usual
reference counting also guarantees that the `bytearray` won't be freed.

It's also possible to implement a Python-style list in Rust, though to make it
work you have to [reference count everything](https://is.gd/tQs5Rd).

## A third way

Most languages roughly follow one of these two approaches. Low-level languages
(C/C++/Rust) allow pointers directly into the memory of their arrays, but they
have to be very careful about mutation as a result. High-level languages
(Python/JS/Java) are more permissive about mutation, but they don't hand out
interior pointers.

One notable exception here is Go, which allows interior pointers *and* makes it
easy to mutate the collections they point into. This has interesting
consequences:

```go
// Create a new list and take a pointer to its first element.
mylist := []string{"a", "b", "c"}
first := &mylist[0]

// We can use the pointer to modify `mylist`.
*first = "a2"
fmt.Printf("%#v\n", mylist) // []string{"a2", "b", "c"}

// Append a new string to the list. This allocates new memory.
mylist = append(mylist, "d")

// The pointer can't modify `mylist` anymore, because it points to old memory.
*first = "a3"
fmt.Printf("%#v\n", mylist) // []string{"a2", "b", "c", "d"}
```

This sort of thing is [illegal in Rust](https://is.gd/mMK1we), but it's similar
to how vectors work in C++, where growing a vector invalidates any existing
pointers. Because Go is garbage collected, you'll get stale data instead of
invoking undefined behavior, but the result is probably still going to cause
bugs.

This kind of slice behavior in Go is tricky, and it might be one reason the Go
developers decided to make slices a [value
type](https://blog.golang.org/slices#TOC_4.) instead of a reference type, and
to rely on [`append` tricks](https://github.com/golang/go/wiki/SliceTricks)
instead of defining methods for things like insert and delete. The `a =
append(a, ...)` syntax is kind of awkward, but it does highlight that you're
getting a *new* slice instead of modifying the one you had before.

Note also that unlike slices, maps in Go are [*not*
addressable](http://devs.cloudimmunity.com/gotchas-and-common-mistakes-in-go-golang/index.html#map_value_field_update).
You can't take pointers to the values inside them.


Thoughts

- Python lets you do the for loop
- Rust doesn't
- the reason is that Rust points to interior memory
- GC'd languages try to avoid defining ownership, but that means that interior
  memory can't be exposed.
  - Is this really true? I could take something out of foo.bar, and then foo
    could swap its bar pointer out, and I would have the wrong thing.
    - Yes it is true! I can get my hands on foo.bar, but I *can't* get &foo.bar
      ("the place where a bar would live inside of foo"). So for example, if I
      have many different types of objects (or fields of a single object) that
      might hold a bar, and I want a list of pointers to several bar-holding
      spots for writing, I can't make that list. I would have to use closures
      that refer to parent objects, or something like that.
- Go is an unusual exception.
