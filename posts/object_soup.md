# In Rust, Object Soup is Made of Indexes
###### DRAFT

When the objects in a program all point back and forth to each other, I call
that "object soup". Object soup is hard to write in Rust, because the borrow
checker doesn't like it. The most common example of this problem is linked
lists, and there's a [whole
book](https://rust-unofficial.github.io/too-many-lists) about how to write
them. But sometimes the problem isn't specific to one data structure. What do
we do when our entire program is object soup, like a game or a simulation?

The short answer is, we need to **use indexes instead of references**. To see
how this works, let's write a toy example in Python and try to port it to Rust.

## Part One: Clones

In our toy example, we'll define a `Person` type, and each `Person` will have a
list of `friends`. These people will be our object soup. We'll write a couple
methods for adding friends and interacting with them, and then we'll test those
methods on Alice and Bob. Here's the Python version
([Godbolt](https://godbolt.org/z/1q4c99cYq)):
```python
class Person:
    def __init__(self, name):
        self.name = name
        self.friends = []

    def add_friend(self, other):
        self.friends.append(other)

    def greet_friends(self):
        for friend in self.friends:
            print(f'{self.name} says, "Hello, {friend.name}!"')

alice = Person("Alice")
bob = Person("Bob")
alice.add_friend(bob)
bob.add_friend(alice)
alice.greet_friends()
bob.greet_friends()
```

This prints:

```
Alice says, "Hello, Bob!"
Bob says, "Hello, Alice!"
```

Cool. In Python, there's nothing special or interesting about this code. Now
let's try to port it to Rust. Here's a first draft, which doesn't compile
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=d86e16b73517f58b96d755487407a7bd)):

```rust
struct Person {
    pub name: String,
    pub friends: Vec<Person>,
}

impl Person {
    fn new(name: &str) -> Person {
        Person { name: name.into(), friends: Vec::new() }
    }

    fn add_friend(&mut self, other: Person) {
        self.friends.push(other);
    }

    fn greet_friends(&self) {
        for friend in &self.friends {
            println!("{} says, \"Hello, {}!\"", self.name, friend.name);
        }
    }
}

fn main() {
    let mut alice = Person::new("Alice");
    let mut bob = Person::new("Bob");
    alice.add_friend(bob);
    bob.add_friend(alice); // error: borrow of moved value: `bob`
    alice.greet_friends();
    bob.greet_friends();
}
```

The compiler is upset. What's the matter?

```
error[E0382]: borrow of moved value: `bob`
  --> src/main.rs:26:5
   |
24 |     let mut bob = Person::new("Bob");
   |         ------- move occurs because `bob` has type `Person`, which
   |                 does not implement the `Copy` trait
25 |     alice.add_friend(bob);
   |                      --- value moved here
26 |     bob.add_friend(alice); // error: borrow of moved value: `bob`
   |     ^^^^^^^^^^^^^^^^^^^^^ value borrowed here after move
```

Ok, the `Person` type isn't `Copy`, so we can't pass it by value to
`add_friend` if we want to keep using it. No problem, we can `#[derive(Clone)]`
on `Person` and call add friends like this
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=c220292da9b52e878629e47fa49300a7)):

```rust
alice.add_friend(bob.clone());
bob.add_friend(alice.clone());
```

Now the compiler is happy, and the output is the same. But those calls to
`.clone()` probably don't feel right. Each `Person` is going to hold copies of
all its friends. That works in this simple case, but it's definitely not going
to work if Alice and Bob change in any way over time.[^already_incorrect] For
example, let's change Alice and Bob's names partway through. The last few lines
of Python can be ([Godbolt](https://godbolt.org/z/18EneYMGM)):

[^already_incorrect]: In fact it's already incorrect. Bob's friend list changed
    when we called `bob.add_friend`, but the copy of Bob in Alice's friends
    list doesn't reflect that.

```python
alice.greet_friends()
alice.name = "Charlotte"
bob.name = "Doug"
bob.greet_friends()
```

Now the ouput becomes:

```
Alice says, "Hello, Bob!"
Doug says, "Hello, Charlotte!"
```

We can make the same change in Rust, and the code will compile
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=847b0c7b52821bb3cc5e3f8749ed7c5e)):

```rust
alice.greet_friends();
alice.name = "Charlotte".into();
bob.name = "Doug".into();
bob.greet_friends();
```

But the output isn't going to match:

```
Alice says, "Hello, Bob!"
Doug says, "Hello, Alice!"
```

Sure enough, Bob is now Doug, but Doug's `friends` list doesn't know that Alice
changed her name. Python didn't have this problem, because everything in Python
happened "by reference" instead of making copies. How do we avoid making copies
in Rust?

## Part Two: References

At this point, it's common for Rust beginners to try to use references, but
Rust references on their own fundamentally cannot model this program. If
`friends` was a list of `&Person`, then no friend could ever be mutated, either
through a friend reference or through any other alias.[^from_utf8] But if
`friends` was a list of `&mut Person`, then aliasing wouldn't be allowed at
all, and no two people could ever have a friend in common.[^lifetime_issues]

[^from_utf8]: Without this rule, a function like
    [`str::from_utf8`](https://doc.rust-lang.org/std/str/fn.from_utf8.html)
    wouldn't be sound, because you might get a `&str` from a `&[u8]` and then
    invalidate those bytes somehow afterwards. This is also why [memory mapping
    a file is fundamentally
    unsafe](https://docs.rs/memmap2/latest/memmap2/struct.Mmap.html#method.map);
    there's no way to prevent mutable aliasing.

[^lifetime_issues]: If we could set aside the aliasing and mutability rules, we
    would also quickly run into pure lifetime issues, because a reference can't
    outlive the object it refers to. In fact it's possible to [get a toy
    example
    working](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=ac564ee9fb4346c6b4d697b5b33b14cf)
    using shared references and
    [`RefCell`](https://doc.rust-lang.org/std/cell/struct.RefCell.html), but
    this party trick stops compiling as soon as we [move any of the
    objects](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=6e886f6e70c1ec59017f9cb79bae976f),
    so it isn't useful in real programs.

Unfortunately, Rust's syntax for reference lifetimes is complicated, and it's
hard for beginners to tell the difference between syntax mistakes and these
fundamental limitations. Here's [a syntactically valid Playground
example](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=963add20fc7ef1b52344eb811809fe5d)
of trying to do this with references and running into unsolvable compiler
errors like this one:

```
error[E0502]: cannot borrow `bob` as mutable because it is also borrowed
              as immutable
  --> src/main.rs:26:5
   |
25 |     alice.add_friend(&bob);
   |                      ---- immutable borrow occurs here
26 |     bob.add_friend(&alice);
   |     ^^^^^^^^^^^^^^^^^^^^^^ mutable borrow occurs here
27 |     alice.greet_friends();
   |     --------------------- immutable borrow later used here
```

The reason we stopped using clones was that we wanted it to be possible for a
`Person` to "change out from under us". That is indeed how references and
pointers work in most languages, but it's not how they work in Rust. We need a
different approach.

## Part Three: Interior Mutability

If you Google "how to mutate a shared object in Rust", you'll find articles
about [`Rc`](https://doc.rust-lang.org/std/rc/struct.Rc.html) and
[`RefCell`](https://doc.rust-lang.org/std/cell/struct.RefCell.html).[^arc_mutex]
This is usually the next stop for beginners who run into the reference
limitations above. But in my opinion, **`Rc<RefCell<T>>` is usually the wrong
approach.** It can work, but it's a pain, and some of its downsides don't show
up until our program gets bigger.

[^arc_mutex]: You will also find articles about
    [`Arc`](https://doc.rust-lang.org/std/sync/struct.Arc.html) and
    [`Mutex`](https://doc.rust-lang.org/std/sync/struct.Mutex.html). These are
    important and useful tools for multithreaded code, but in a single-threaded
    context they're just more expensive versions of `Rc` and `RefCell`.

In this approach, the type of `friends` becomes `Vec<Rc<RefCell<Person>>>`, and
the main function of our program looks like this
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=49a48c8910d4f1abd40a7c6733e0964b)):

```rust
let alice = Rc::new(RefCell::new(Person::new("Alice")));
let bob = Rc::new(RefCell::new(Person::new("Bob")));
alice.borrow_mut().add_friend(Rc::clone(&bob));
bob.borrow_mut().add_friend(Rc::clone(&alice));
alice.borrow().greet_friends();
alice.borrow_mut().name = "Charlotte".into();
bob.borrow_mut().name = "Doug".into();
bob.borrow_mut().greet_friends();
```

Finally, our Rust program is at least printing the right output:

```
Alice says, "Hello, Bob!"
Doug says, "Hello, Charlotte!"
```

Typing `borrow` or `borrow_mut` every single time we mention an object is a
drag, but if that was the only downside, maybe we could live with it. In fact,
there are bigger downsides. The contract of `borrow_mut` is that it will panic
if the `RefCell` is already borrowed, to avoid breaking the aliasing rules. As
our program expands, we'll start to run into these panics. Let's add a little
more code to the Python version, to print a message whenever we create a mutual
friendship ([Godbolt](https://godbolt.org/z/aedGhEb6q)):

```python
def add_friend(self, other):
    self.friends.append(other)
    for other_friend in other.friends:
        if other_friend.name == self.name:
            print(f"Mutual friends!")
```

This works fine in Python:[^same_name]

[^same_name]: As long as no two people have the same name. It's fine.

```
Mutual friends!
Alice says, "Hello, Bob!"
Doug says, "Hello, Charlotte!"
```

The changing names are a little weird, forgive me, but that's the output we
asked for. What about Rust?
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=f00b4de714afe65ec8961b92e222bc54))

```rust
fn add_friend(&mut self, other: &Rc<RefCell<Person>>) {
    self.friends.push(Rc::clone(other));
    for other_friend in &other.borrow().friends {
        if other_friend.borrow().name == self.name {
            println!("Mutual friends!");
        }
    }
}
```

This panics:

```
thread 'main' panicked at 'already mutably borrowed: BorrowError',
src/main.rs:17:29
note: run with `RUST_BACKTRACE=1` environment variable to display
a backtrace
```

The problem is that the caller used `borrow_mut` to get the `&mut self`
reference used to call this method, and that exclusive borrow is still active.
When one of the `other_friend` entries is an aliasing reference back to us, the
call to `borrow` conflicts with the active `borrow_mut`.

To avoid this issue, we can use a different coding style that prefers
freestanding functions over methods and keeps all `RefCell` borrows local. It
looks like this
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=1cf6572935f5152ae26579f777532cde)):

```rust
fn add_friend(this: &Rc<RefCell<Person>>, other: &Rc<RefCell<Person>>) {
    this.borrow_mut().friends.push(Rc::clone(other));
    for other_friend in &other.borrow().friends {
        if other_friend.borrow().name == this.borrow().name {
            println!("Mutual friends!");
        }
    }
}
```

The biggest downside of this approach is that the compiler doesn't have our
back. When we mix normal, idiomatic Rust code in with this new style, we'll
probably cause bugs, but we won't find out about them until runtime, and even
then only if our test coverage is good. This isn't the level of confidence we
expect from Rust.

## Part Four: Indexes

We can do better with simpler tools. Here's the trick
([Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=d4d6fda7715853ce51f43620b8f275e5)):

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
    people[this_id].friends.push(other_id);
    for &other_friend in &people[other_id].friends {
        if people[other_friend].name == people[this_id].name {
            println!("Mutual friends!");
        }
    }
}

fn greet_friends(people: &Vec<Person>, this_id: usize) {
    for &friend_id in &people[this_id].friends {
        println!(
            "{} says, \"Hello, {}!\"",
            people[this_id].name, people[friend_id].name,
        );
    }
}

fn main() {
    let mut people = Vec::new();
    let alice_id = new_person(&mut people, "Alice");
    let bob_id = new_person(&mut people, "Bob");
    add_friend(&mut people, alice_id, bob_id);
    add_friend(&mut people, bob_id, alice_id);
    greet_friends(&people, alice_id);
    people[alice_id].name = "Charlotte".into();
    people[bob_id].name = "Doug".into();
    greet_friends(&people, bob_id);
}
```

As in Part Three, we've replaced inherent methods on `Person` with standalone
functions. The compiler has our back this time, and if we make any mistakes by
mixing styles, those mistakes will be compiler errors.

## Part Five: Growing From Here

The most obvious missing feature here is that we can't delete elements without
messing up the indexes of subsequent elements. In a small program where
performance isn't critical and taking a dependency feels excessive, one way to
support deletion is to replace `Vec<People>` with `HashMap<u64, People>`, using
an incrementing counter for new indexes. In a larger program like a game or a
simulation, we can use a specialized data structure like
[`slotmap`](https://docs.rs/slotmap/latest/slotmap/index.html) for this.

In a real program we'll probably have more types than just `Person`, and we'll
want to group all of our containers into some larger struct and call it
something like `World` or `Context` or `Entities`. At this point our design
might start to look like an ["entity component
system"](https://en.wikipedia.org/wiki/Entity_component_system), a common
design pattern for games in any language. [Catherine West's 2018 keynote on
game development in Rust](https://www.youtube.com/watch?v=aKLntZcp27M) is
mandatory viewing on this subject.

A nice side-effect of using indexes instead of references is that our object
soup is now much easier to serialize. We could
[`#[derive(Serialize)]`](https://serde.rs/derive.html) on our `World` struct,
encode is as JSON, and save it to a file. If our program is less of a game and
more of a network service, we could replace `Vec` or `SlotMap` with a table in
a database. This refactoring will be easier than usual, since our functions
already refer to objects by ID and take a context object from the caller.

Another nice side-effect is that a large `Vec` or `SlotMap` of objects usually
performs better than if all those objects were allocated individually.

One of Catherine West's most important points in her keynote above is that
programs in all languages tend to converge on these same design patterns. The
difference in Rust is that you need to adopt those patterns earlier, even when
your programs are small and object soup would usually be good enough. That's a
learning tax, but it's also a learning investment.
