# Object Soup is Made of Indexes
###### DRAFT

When objects come and go and change unpredictably, and their relationships are
also changing and full of cycles, I call it "object soup". It's hard to write
object soup in Rust, because it breaks the rules for references.[^the_rules]
But sometimes it's just how the world works: A creature in a game targets
another creature, and then its target disappears. A cell in a spreadsheet
depends on another cell, and then the other cell's value changes. A song in a
music player links to the singer, and the singer links to their songs. These
programs are object soup by design, but Rust doesn't let us do things like this
with references. So what do we do?

[^the_rules]: I'm assuming that you've already seen Rust's ownership,
    borrowing, and mutability rules. If not, here's [an overview from a talk by
    Niko Matsakis](https://www.youtube.com/watch?v=lO1z-7cuRYI&t=1302), and
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

This is simple, boring Python, and it would be simple and boring in most
languages.[^dark_corners] But in Rust it's surprisingly tricky.

[^dark_corners]: Garbage collection solves a lot of problems, but there are
    [monsters
    lurking](https://docs.python.org/reference/datamodel.html#object.__del__)
    in [dark corners](https://openjdk.org/jeps/421).

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

Passing Bob to `add_friend` by value moves him, because `Person` isn't
`Copy`.[^move_semantics] A quick fix is to add `#[derive(Clone)]` to `Person`
and to `clone` each argument to `add_friend`
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=3d1931a063c9a404233e5f8ff4e68c87)).
But copying or cloning isn't what we want when we're writing object soup. The
real Alice and Bob will change over time, and any copies of them will quickly
get out of sync.[^already_wrong]

[^move_semantics]: Again I'm assuming that you've already seen move semantics
    in Rust. If not, here's [the relevant chapter of The
    Book](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html#variables-and-data-interacting-with-move),
    and here's [a comparison with move semantics in C++ from one of my
    talks](https://www.youtube.com/watch?v=IPmRDS0OSxM&t=3020).

[^already_wrong]: In fact, one of the clones in this example is already out of
    sync. The copy of Bob in Alice's friends list [doesn't get
    updated](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=e8bc304b0542b5db3cb8855912c197de)
    by the second call to `add_friend`.

Like most garbage-collected languages, Python doesn't have this problem,
because it passes objects around "by reference". Can we use references in Rust?

## Part Two: Borrowing

No we can't, because Rust doesn't let us mutate objects that are
borrowed.[^interior_mutability] If we use shared references
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=86e3feb50c34e1659ca614c536ac512d)):

[^interior_mutability]: The exception to this rule is "interior mutability",
    and we'll get to that in the next section.

```rust
alice.add_friend(&bob);
bob.add_friend(&alice);
```
We'll get a compiler error when we try to modify Bob:

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

If we use mutable references
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=f4036832326cf76f4e72e7fa8f18868c)),
we can avoid aliasing Bob by going through Alice's friends list to modify
him:[^many_to_many]

[^many_to_many]: The uniqueness rule means we can't use mutable references for
    many-to-many relationships, so we definitely can't make object soup out of
    them in general.

```rust
alice.add_friend(&mut bob);
alice.friends[0].add_friend(&mut alice);
```

But we'll still get an error about aliasing Alice:

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

Playing with these examples is educational,[^advice] but there's no way to make
them work.[^party_trick] Object soup wants aliasing and mutation at the same
time, and that's exactly what references in Rust are designed to prevent. We
need something different.

[^party_trick]: Ok I lied. You can get something working [by combining shared
    references and interior
    mutability](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=cee3ebe1debd9f241a8159b3203051ea).
    Circular borrows in safe code! It's a neat party trick, but it's not useful
    in real programs, because it [breaks if you try to move
    anything](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=1c56892372f33c4e6e492ada3000571e).

[^advice]: It's worth spending some time "fighting the borrow checker" to build
    up intuition about what works and what doesn't. But when you get stuck, a
    good rule of thumb is to avoid putting lifetime parameters on structs.

## Part Three: Interior Mutability

If you search for "how to mutate a shared object in Rust", you'll find articles
about [`Rc`](https://doc.rust-lang.org/std/rc/struct.Rc.html)[^rc] and
[`RefCell`](https://doc.rust-lang.org/std/cell/struct.RefCell.html),[^refcell]
but **`Rc<RefCell<T>>` doesn't work well for object soup.** To see why, let's
try it
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=f47eed8da77a7bf5801679639ff9c6c9)):

[^rc]: `Rc` stands for ["reference
    counting"](https://en.wikipedia.org/wiki/Reference_counting), which is the
    strategy it uses to free its contents. It behaves like a shared reference
    with no lifetime. It's similar to `std::shared_ptr` in C++ and automatic
    reference counting in Swift.

[^refcell]: `RefCell` is like an
    [`RwLock`](https://doc.rust-lang.org/stable/std/sync/struct.RwLock.html)
    that panics instead of blocking and can't be shared across threads. It lets
    us get `&mut T` from `&RefCell<T>` (which we get from `Rc`).

```rust
let alice = Rc::new(RefCell::new(Person::new("Alice")));
let bob = Rc::new(RefCell::new(Person::new("Bob")));
alice.borrow_mut().add_friend(Rc::clone(&bob));
bob.borrow_mut().add_friend(Rc::clone(&alice));
```

There's a lot going on there,[^a_lot_going_on] and it's pretty verbose, but it
compiles and runs. Progress! Unfortunately, if we run it under ASan
([Godbolt](https://godbolt.org/z/dE6s5qKes)), we see that it's leaking
memory.[^difficult_to_leak] To fix that, we either need to explicitly break
cycles before Alice and Bob go out of scope
([Godbolt](https://godbolt.org/z/G8z4sjPW6)), or we need to use
[`Weak`](https://doc.rust-lang.org/std/rc/struct.Weak.html) references
([Godbolt](https://godbolt.org/z/GTo3svrY8)). Both options are
error-prone.[^weak_semantics]

[^a_lot_going_on]: `borrow_mut` returns a [smart
    pointer](https://doc.rust-lang.org/book/ch15-00-smart-pointers.html) type
    called [`RefMut`](https://doc.rust-lang.org/std/cell/struct.RefMut.html)
    that implements the
    [`Deref`](https://doc.rust-lang.org/std/ops/trait.Deref.html) and
    [`DerefMut`](https://doc.rust-lang.org/std/ops/trait.DerefMut.html) traits.
    A lot of Rust magic works through those traits and ["deref
    coercions"](https://doc.rust-lang.org/book/ch15-02-deref.html). [Spelling
    out all the
    types](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=d2d483057befb89da871ac7fc009f153)
    is helpful for seeing what's going on. The same pattern comes up with
    `Arc<Mutex<T>>`, which is fundamental for multithreading.

[^difficult_to_leak]: Usually it's hard to leak memory by accident in Rust, but
    reference cycles in `Rc` and `Arc` are the main exception. Again this is
    similar to C++ and Swift, and the same thing happens in Python if you call
    [`gc.disable`](https://docs.python.org/3/library/gc.html#gc.disable).

[^weak_semantics]: `Weak` references are a good fit for asymmetrical
    relationships like child nodes and parent nodes in a tree, but "strong
    friends" and "weak friends" don't really make sense.

As our program grows, the uniqueness rule will also come back to bite us in the
form of `RefCell` panics. To provoke that, let's change `add_friend` to check
for people befriending themselves. Here's the change in Python
([Godbolt](https://godbolt.org/z/EY7xe545j)):[^same_name]

```python
def add_friend(self, other):
    if other.name != self.name:
        self.friends.append(other)
```

[^same_name]: No two people ever have the same name. It's fine.

And in Rust:

```rust
fn add_friend(&mut self, other: &Rc<RefCell<Person>>) {
    if other.borrow().name != self.name {
        self.friends.push(Rc::clone(other));
    }
}
```

The Rust version compiles, but if we make Alice call `add_friend` on herself,
it panics
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=732713c6149a652504e4ed7160c1fd64)):

```
thread 'main' panicked at 'already mutably borrowed: BorrowError',
src/main.rs:15:18
```

The problem is that we "locked" the `RefCell` to get `&mut self`, and that
conflicts with `other.borrow()` when `other` is aliasing `self`. The fix is to
avoid `&mut self` methods and keep our borrows short-lived
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=e5e9ce34c0d8eace668a542a9e91f8c9)),
but this is also error-prone. We might've missed this bug without a test case.

`Rc<RefCell<T>>` isn't a good way to write object soup, because it has problems
with aliasing and cycles.[^unsafe_code] Again we need something different.

[^unsafe_code]: Unsafe code has similar problems. Unless you're extremely
    careful, raw pointer soup usually breaks the uniqueness rule when you
    convert pointers back into references to call safe functions. That's
    undefined behavior in Rust, [even when the same code would've been legal in
    C or C++](https://www.youtube.com/watch?v=DG-VLezRkYQ).

## Part Four: Indexes

It turns out we can do better with simpler tools. We can keep Alice and Bob in
a `Vec` and have them refer to each other by index
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=edc7d9ebf27a9785e8ac7cc2a8e32296)):

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
    add_friend(&mut people, alice_id, alice_id); // no-op
}
```

Some of the verbosity from the `RefCell` approach is still here: we're avoiding
`&mut self` methods, and each function has a new `people` argument. But unlike
above, aliasing mistakes are compiler errors instead of runtime panics
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=e81e266eca0254b2488aa76a99eac4f4)),
and there's no risk of memory leaks
([Godbolt](https://godbolt.org/z/hfK5bMTav)). This is how you write object soup
in Rust.

## Part Five: Next Steps

Even though we're technically not leaking memory, we still can't delete
anything from the `Vec` without messing up the indexes of other elements. One
way to allow for deletion is to replace `Vec` with `HashMap`, using either an
incrementing counter or [random UUIDs](https://docs.rs/uuid) for the keys. If
you need better performance and you don't mind taking a dependency, there are
also specialized data structures like [`Slab`](https://docs.rs/slab) and
[`SlotMap`](https://docs.rs/slotmap).

When you have more than one type of object to keep track of, you'll probably
want to group them in a struct with a name like `World` or `State` or
`Entities`. In her [2018 keynote on writing games in
Rust](https://www.youtube.com/watch?v=aKLntZcp27M),[^inspiration] Catherine
West talked about how this pattern is a precursor to what game developers call
an "entity component system". These patterns solve borrowing and mutability
problems in Rust, but they also solve problems like serialization,
synchronization, and cache locality that come up when we write object soup in
any language.

[^inspiration]: This article is really just a rehash of Catherine's talk. I
    can't recommend it highly enough.
