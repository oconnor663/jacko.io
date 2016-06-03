# Iterator invalidation in Rust

## or: ARGH why can't this just work like it does in Python?

Even as I get more familiar with Rust, I still spend some time "fighting the
borrow checker". On a good day, maybe I just need to reorder some variables, or
throw in a pair of curly braces. But on a bad day, what I've written is just
deeply offensive to the compiler, and nothing I do is going to make it ok. It's
especially annoying when I know what I've written is fine in other langauges.
And *especially* especially annoying when all I'm doing is iterating over a
list.

Here's something that Python is perfectly willing to let me do with a list:

```python
mylist = [1, 2, 3]
for i in mylist:
    print(i)
    if i == 2:
        mylist.append(4)

# 1
# 2
# 3
# 4
```

Sure, I can modify a list while I'm iterating over it. It might feel a little
dirty, but anyway the meaning is clear. "When you get to 2, append the 4, then
keep on going." Simple.

Rust disagrees.

```rust
let mut mylist = vec![1, 2, 3];
for i in &mylist {
    println!("{}", i);
    if *i == 2 {
        mylist.push(4); # ERROR: cannot borrow `mylist` as mutable
    }                   # because it is also borrowed as immutable
}

```

Aww c'mon Rust! Why does this have to be so hard? I know, it's Against The
Rules for anything to alias a mutable reference. But it feel like such an
arbitrary limitation right now. Why can't you just do what Python does? It's
not like this is going to cause *undefined behavior*...is it?

Yes. Yes it is, and that's exactly why Rust doesn't let you do it.

The key difference between Python and Rust in these examples is the variable
`i`. In Python, `i` points to an integer that lives somewhere on its own, and
if `mylist` disappeared, `i` would still be perfectly valid. In Rust however,
`i` points to an integer *inside* of `mylist`'s memory. If Rust let us do
`mylist.push(4)`, then `mylist` would need to grow, its memory would move
around, and `i` would be a dangling pointer!
