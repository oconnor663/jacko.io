# Iterator invalidation in Rust

## or: ARGH why doesn't this just work like in Python?

Even as I get more familiar with Rust, I still spend some time "fighting the
borrow checker". On a good day, maybe I just need to reorder some variables, or
throw in a pair of curly braces, or (G-d help me) use explicit lifetimes. But
on a bad day, I've written something that's deeply offensive to the compiler,
and nothing I do is going to change that. It's especially annoying when I know
what I'm doing is fine in other langauges. And it's *especially* especially
annoying when all I'm doing is iterating over a list.

Here's something that Python is more than happy to run for me:

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

Modifying a list while you're iterating over it? I might not be proud of
writing that, but sometimes it gets the job done. Anyway, the meaning is clear.
"When you get to 2, append the 4, then keep going." Simple, right?

Rust disagrees.

```rust
let mut mylist = vec![1, 2, 3];
for i in &mylist {
    println!("{}", i);
    if *i == 2 {
        mylist.push(4); # error: cannot borrow `mylist` as mutable
    }                   # because it is also borrowed as immutable
}

```

Aww *c'mon* Rust! Why does this have to be so hard? I know, it's Against The
Rules for anything to alias a mutable reference. But it feel like such an
arbitrary limitation right now. Why can't you just do what Python does? It's
not like this is going to cause *undefined behavior*...is it?

Yes. Yes it is, and that's exactly why Rust doesn't let you do it.

The key difference between Python and Rust in these examples is the variable
`i`. In Python, `i` is pointing to an integer object that has its own life
