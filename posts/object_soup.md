# Object Soup is Made of Indexes
###### DRAFT

When objects come and go and change all the time, and their relationships are
also changing and full of cycles, I call it "object soup". It's hard to write
object soup in Rust, because it breaks the rules for references.[^the_rules]
But sometimes it's how the world works: A creature in a game can target another
creature, but its target might disappear. A cell in a spreadsheet can depend on
another cell, and then the other cell's value might change. A playlist has many
songs, and each song can be in many lists. These programs are object soup by
design, but Rust won't let us build them out of references. So how do we build
them?

[^the_rules]: I'm assuming that you've already seen Rust's ownership,
    borrowing, and mutability rules. If not, here's here's [an overview from a
    talk by Niko
    Matsakis](https://youtu.be/lO1z-7cuRYI?si=fumjE3ee_cJTJBuF&t=1302), and
    here's [the relevant chapter of The
    Book](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html).

The short answer is, we **use indexes instead of references**. To see why,
we'll start by looking at three other approaches that don't work. If you just
want to see the code that works, skip to part four.

Our object soup of the day is a toy program that models two friends, Alice and
Bob. Here's the Python version ([Godbolt](https://godbolt.org/z/cdMjoqGc7)):

```python
class Person:
    def __init__(self, name):
        self.name = name
        self.friends = []

    def add_friend(self, other):
        self.friends.append(other)

alice = Person("Alice")
bob = Person("Bob")
alice.add_friend(bob)
bob.add_friend(alice)
```

That's it. It's simple, boring Python, and it would be simple and boring in
most other languages. But in Rust it's tricky.[^other_languages]

[^other_languages]: Some of the issues below come up in any language that
    doesn't have a background garbage collector, including C, C++, and Swift.
    The lifetime and aliasing rules are also similar to the rules we have to
    follow when we write multithreaded code in any language. But Rust is
    somewhat unique in forcing us to solve these issues even in single-threaded
    toy examples.

## Part One: Move Semantics

A naive Rust translation doesn't compile
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=e78b6cc6d878ff7226a33f8697a0c5f5)):

```rust
struct Person {
    name: String,
    friends: Vec<Person>,
}

impl Person {
    fn new(name: &str) -> Person {
        Person { name: name.into(), friends: Vec::new() }
    }

    fn add_friend(&mut self, other: Person) {
        self.friends.push(other);
    }
}

fn main() {
    let mut alice = Person::new("Alice");
    let mut bob = Person::new("Bob");
    alice.add_friend(bob);
    bob.add_friend(alice); // error: borrow of moved value: `bob`
}
```

Here's the full error:

```
error[E0382]: borrow of moved value: `bob`
  --> src/main.rs:20:5
   |
18 |     let mut bob = Person::new("Bob");
   |         ------- move occurs because `bob` has type `Person`, which
   |                 does not implement the `Copy` trait
19 |     alice.add_friend(bob);
   |                      --- value moved here
20 |     bob.add_friend(alice); // error: borrow of moved value: `bob`
   |     ^^^^^^^^^^^^^^^^^^^^^ value borrowed here after move
```

Passing `bob` to `add_friend` by value moves him, because `Person` is not
`Copy`.[^move_semantics] A quick fix is to add `#[derive(Clone)]` to `Person`
and to `.clone()` each argument to `add_friend`
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=67ffa8e875c6c518bccbcb83202dbbb3)).
But cloning is a "deep copy", and that's not what we want when we're writing
object soup. The real Alice and Bob are going to change over time, and copies
of them will quickly get out of sync. [^already_wrong]

[^move_semantics]: Again I'm assuming that you've already seen move semantics
    in Rust. If not, here's [the relevant chapter of The
    Book](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html#variables-and-data-interacting-with-move),
    and here's [a comparison with move semantics in C++ from one of my
    talks](https://www.youtube.com/watch?v=IPmRDS0OSxM&t=3020).

[^already_wrong]: In fact, one of the copies has already gotten out of sync
    even in that tiny Playground example. The copy of Bob in Alice's friends
    list [didn't get
    updated](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=7b0b02c01d8b5d5e093ea5e4421ec80c)
    by the second call to `add_friend`.

Like most garbage-collected languages, Python doesn't have this problem,
because it passes objects around "by reference". Can we use references in Rust?

## Part Two: Borrowing

No we can't, because Rust doesn't let us mutate objects that are
borrowed.[^interior_mutability] If the friends list was holding shared
references, [as in this Playground
example,](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=86e3feb50c34e1659ca614c536ac512d)
we'd get a compiler error when we tried to modify Bob:

[^interior_mutability]: The exception to this rule is "interior mutability",
    and we'll get to that in the next section.

```
error[E0502]: cannot borrow `bob` as mutable because it is also borrowed
              as immutable
  --> src/main.rs:20:5
   |
19 |     alice.add_friend(&bob);
   |                      ---- immutable borrow occurs here
20 |     bob.add_friend(&alice);
   |     ^^^^----------^^^^^^^^
   |     |   |
   |     |   immutable borrow later used by call
   |     mutable borrow occurs here
```

Switching to mutable references doesn't help.[^many_to_many] We could avoid
taking a second reference to Bob by going through Alice's friends list to
modify him, [as in this second Playground
example](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=f4036832326cf76f4e72e7fa8f18868c),
but we'd still get an error for breaking the uniqueness rule with respect to
Alice:

[^many_to_many]: The uniqueness rule means we can never use mutable references
    for many-to-many relationships, so it's not surprising that we can't make
    object soup out of them in general. But in fact even this toy example is
    too much.

```
error[E0499]: cannot borrow `alice` as mutable more than once at a time
  --> src/main.rs:20:33
   |
20 |     alice.friends[0].add_friend(&mut alice);
   |     -------------    ---------- ^^^^^^^^^^ second mutable borrow
   |     |                |                     occurs here
   |     |                first borrow later used by call
   |     first mutable borrow occurs here
```

It's educational to play with these examples, but there's no way to make them
work.[^party_trick] Object soup is all about doing aliasing and mutation at the
same time, which is exactly what references in Rust are designed to
prevent.[^advice] We need a different approach.

[^party_trick]: Ok I lied. You can make it work [by combining shared references
    and interior
    mutability](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=5062e6d7f9d3ba52b69140a73614f796).
    Circular borrows in safe code! It's a neat party trick, but it's not useful
    in real programs, because it [breaks if you move
    anything](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=1c56892372f33c4e6e492ada3000571e).

[^advice]: Unfortunately, lots of people go through a painful phase of
    "fighting the borrow checker" while they learn these rules. My advice for
    beginners is, no matter what the compiler says, don't put any lifetimes
    parameters on structs.

## Part Three: Interior Mutability

If you Google "how to mutate a shared object in Rust", you'll find articles
about [`Rc`](https://doc.rust-lang.org/std/rc/struct.Rc.html) and
[`RefCell`](https://doc.rust-lang.org/std/cell/struct.RefCell.html).[^arc_mutex]
In short, `Rc` is like a shared reference without a
lifetime,[^reference_counting] and `RefCell` lets you get a mutable reference
from a shared one.[^flag] But in my opinion, **`Rc<RefCell<T>>` is an
anti-pattern.** It can work in small doses, but it's not a good way to organize
your whole program.

[^arc_mutex]: You'll also find articles about
    [`Arc`](https://doc.rust-lang.org/std/sync/struct.Arc.html) and
    [`Mutex`](https://doc.rust-lang.org/std/sync/struct.Mutex.html). These are
    important and useful tools for multithreaded code, but in single-threaded
    code they're just more expensive versions of `Rc` and `RefCell`.

[^reference_counting]: `Rc` stands for ["reference
    counted"](https://en.wikipedia.org/wiki/Reference_counting), which is the
    strategy it uses to free its contents. It's similar to `std::shared_ptr` in
    C++ and also similar to how Python works under the hood.

[^flag]: `RefCell` is essentially an
    [`RwLock`](https://doc.rust-lang.org/stable/std/sync/struct.RwLock.html)
    that doesn't support blocking and can't be shared across threads. Since it
    doesn't require any OS support, it's [available in
    `core`](https://doc.rust-lang.org/core/cell/index.html).

Here's what it looks like when we use it
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=48ce017229dd12f24d5999830b172985)):

```rust
use std::cell::RefCell;
use std::rc::Rc;

struct Person {
    name: String,
    friends: Vec<Rc<RefCell<Person>>>,
}

impl Person {
    fn new(name: &str) -> Person {
        Person { name: name.into(), friends: Vec::new() }
    }

    fn add_friend(&mut self, other: Rc<RefCell<Person>>) {
        self.friends.push(other);
    }
}

fn main() {
    let alice = Rc::new(RefCell::new(Person::new("Alice")));
    let bob = Rc::new(RefCell::new(Person::new("Bob")));
    alice.borrow_mut().add_friend(Rc::clone(&bob));
    bob.borrow_mut().add_friend(Rc::clone(&alice));
}
```

This compiles and runs on the Playground, but it has a memory
leak.[^difficult_to_leak] We can confirm that with ASan
[(Godbolt)](https://godbolt.org/z/65r8xc1jK). To fix it, we either need to
explicitly break cycles before Alice and Bob go out of scope
[(Godbolt)](https://godbolt.org/z/WzqbnaTY7), or we need to use
[`Weak`](https://doc.rust-lang.org/std/rc/struct.Weak.html) references
[(Godbolt)](https://godbolt.org/z/nh3Kq3qd8). Both options are
error-prone.[^weak_semantics]

[^difficult_to_leak]: Usually it's hard to leak memory by accident in Rust, and
    the functions that do it on purpose have obvious names like
    [`Box::leak`](https://doc.rust-lang.org/std/boxed/struct.Box.html#method.leak)
    and [`mem::forget`](https://doc.rust-lang.org/std/mem/fn.forget.html). But
    reference cycles in `Rc` and `Arc` are the exception. Again this is similar
    to `std::shared_ptr` in C++.

[^weak_semantics]: `Weak` references are a good fit for asymmetrical
    relationships, like child pointers (strong) and parent pointers (weak) in a
    tree, but here it's not clear who should be the "strong friend" or the
    "weak friend".

As our program grows, the uniqueness rule will also come back in the form of
`RefCell` panics. To trigger this, let's add a check to `add_friend` to make
sure people don't befriend themselves. Here's the one-line change in Python
([Godbolt](https://godbolt.org/z/EY7xe545j)):

```python
def add_friend(self, other):
    if other.name != self.name:
        self.friends.append(other)
```

Again this is simple, boring Python.[^same_name] What about Rust?

[^same_name]: As long as no two people have the same name. It's fine.

```rust
fn add_friend(&mut self, other: &Rc<RefCell<Person>>) {
    if other.borrow().name != self.name {
        self.friends.push(Rc::clone(other));
    }
}
```

The Rust version compiles, but if we test it by making Alice call `add_friend`
on herself, it panics
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=732713c6149a652504e4ed7160c1fd64)).

```
thread 'main' panicked at 'already mutably borrowed: BorrowError',
src/main.rs:15:18
```

The caller has locked the `RefCell` to give us `&mut self`, and that conflicts
with `other.borrow()` when `other` is also Alice.[^deadlock] We can fix it by
avoiding `&mut self` methods and keeping our borrows short-lived
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=e5e9ce34c0d8eace668a542a9e91f8c9)).
But that's starting to get painfully verbose, and the bigger problem is that
it's error-prone. We needed a test case to catch that panic.

[^deadlock]: If we were using `Arc<Mutex<T>>` instead of `Rc<RefCell<T>>`, this
    would be a deadlock instead.

`Rc<RefCell<T>>` causes problems when our data is full of cycles, and it's not
a good way to write object soup. Again we need a different approach.

## Part Four: Indexes

It turns out that we can do better with simpler tools. We can keep Alice and
Bob in a `Vec` and have them refer to each other by index
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=9d48e173702419dcc7bc4e312ea98912)):

```rust
struct Person {
    name: String,
    friends: Vec<usize>,
}

fn new_person(people: &mut Vec<Person>, name: &str) -> usize {
    people.push(Person { name: name.into(), friends: Vec::new() });
    people.len() - 1
}

fn add_friend(people: &mut Vec<Person>, this_id: usize, other_id: usize) {
    if people[other_id].name != people[this_id].name {
        people[this_id].friends.push(other_id);
    }
}

fn main() {
    let mut people = Vec::new();
    let alice_id = new_person(&mut people, "Alice");
    let bob_id = new_person(&mut people, "Bob");
    add_friend(&mut people, alice_id, bob_id);
    add_friend(&mut people, bob_id, alice_id);
}
```

Some of the verbosity from the `RefCell` approach is still here. We're avoiding
`&mut self` methods, and the `people` parameter shows up everywhere. But unlike
above, mistakes here are compiler errors instead of runtime panics, and there's
no risk of memory leaks. This is how you write object soup in Rust.

## Part Five: Next Steps

While ASan would agree that the code above has no memory leaks
([Godbolt](https://godbolt.org/z/vb35Ya3Ej)), we also can't delete anything
from the `Vec` without messing up the indexes of other elements. One way to
support deletion is to replace `Vec<People>` with `HashMap<u64, People>`, using
an incrementing counter for new indexes. For programs with higher performance
requirements that don't mind taking dependencies, there are specialized data
structures like [`SlotMap`](https://docs.rs/slotmap/latest/slotmap/index.html).

When we have more than one type of object, we'll want to group our containers
into a larger struct and call it something like `World` or `Context` or
`Entities`. At that point our design might start to look like an "entity
component system".[^ecs] [Catherine West's 2018 keynote on game development in
Rust](https://www.youtube.com/watch?v=aKLntZcp27M) is mandatory viewing on this
subject.

[^ecs]: The [ECS design
    pattern](https://en.wikipedia.org/wiki/Entity_component_system) is common
    for games in many languages. This is partly because it's good for
    performance, but it also reflects the fact that lifetime and aliasing
    problems aren't unique to Rust.

Using indexes makes our object soup easier to serialize. We could
[`#[derive(Serialize)]`](https://serde.rs/derive.html) on all our types, encode
the `World` as JSON, and save it to a file. Or if our program is less of a game
and more of a network service, we could replace `Vec` or `SlotMap` with a table
in a database. Our functions already refer to objects by ID and take a context
object from the caller.

One of Catherine West's most important points in her keynote above is that
programs in all languages run into these problems as they grow, and they tend
to converge on similar design patterns. Rust forces us to adopt these patterns
earlier, when out programs are still small. That's a cost in terms of learning
and prototyping, but it has its benefits.
