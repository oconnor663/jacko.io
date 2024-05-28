# Object Soup is Made of Indexes
###### 2023 October 23

When objects come and go and change all the time, and any one might point to
any other, I call that "object soup".[^graph] It's hard to write object soup in
Rust, because it breaks the rules for references.[^the_rules] But sometimes
it's just how the world works: A creature in a game targets another creature,
and then its target disappears. A cell in a spreadsheet depends on another
cell, and the other cell's value changes. A song in a music player links to a
singer, and the singer links to their songs. These programs are object soup by
design, but Rust doesn't let us do things like this with references. So what do
we do?

[^graph]: In other words, object soup is an implicit, heterogeneous, mutable
    graph in a program that might not look like it's trying to build a graph.

[^the_rules]: I'm assuming that you've already seen Rust's ownership,
    borrowing, and mutability rules. If not, here's [an overview from a talk by
    Niko Matsakis](https://www.youtube.com/watch?v=lO1z-7cuRYI&t=1302), and
    here's [the relevant chapter of The
    Book](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html).

The short answer is: **use indexes instead of references**. To see why, we'll
look at three other approaches that don't work. If you just want to see the
code that works, skip to part four.

Our object soup of the day is a toy program that models two friends, Alice and
Bob. Here's [the Python version]:

[the Python version]: <https://godbolt.org/#g:!((g:!((g:!((h:codeEditor,i:(filename:'1',fontScale:14,fontUsePx:'0',j:1,lang:python,selection:(endColumn:22,endLineNumber:12,positionColumn:22,positionLineNumber:12,selectionStartColumn:22,selectionStartLineNumber:12,startColumn:22,startLineNumber:12),source:'class+Person:%0A++++def+__init__(self,+name):%0A++++++++self.name+%3D+name%0A++++++++self.friends+%3D+%5B%5D%0A%0A++++def+add_friend(self,+other):%0A++++++++self.friends.append(other)%0A%0Aalice+%3D+Person(%22Alice%22)%0Abob+%3D+Person(%22Bob%22)%0Aalice.add_friend(bob)%0Abob.add_friend(alice)'),l:'5',n:'0',o:'Python+source+%231',t:'0')),k:50,l:'4',m:48.281033364974206,n:'0',o:'',s:0,t:'0'),(g:!((h:executor,i:(argsPanelShown:'1',compilationPanelShown:'0',compiler:python311,compilerName:'',compilerOutShown:'0',execArgs:'',execStdin:'',fontScale:14,fontUsePx:'0',j:1,lang:python,libs:!(),options:'',overrides:!(),runtimeTools:!(),source:1,stdinPanelShown:'1',wrap:'1'),l:'5',n:'0',o:'Executor+Python+3.11+(Python,+Editor+%231)',t:'0')),header:(),k:50,l:'4',n:'0',o:'',s:0,t:'0')),l:'2',n:'0',o:'',t:'0')),version:4>

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

A [naive Rust translation] doesn't compile:

[naive Rust translation]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=struct+Person+%7B%0A++++name%3A+String%2C%0A++++friends%3A+Vec%3CPerson%3E%2C%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person+%7B%0A++++++++Person+%7B+name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26mut+self%2C+other%3A+Person%29+%7B%0A++++++++self.friends.push%28other%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+alice+%3D+Person%3A%3Anew%28%22Alice%22%29%3B%0A++++let+mut+bob+%3D+Person%3A%3Anew%28%22Bob%22%29%3B%0A++++alice.add_friend%28bob%29%3B%0A++++bob.add_friend%28alice%29%3B+%2F%2F+error%3A+borrow+of+moved+value%3A+%60bob%60%0A%7D

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
and to [`clone` each argument to `add_friend`][each_argument]. But copying or
cloning isn't what we want when we're writing object soup. The real Alice and
Bob will change over time, and any copies of them will quickly get out of
sync.[^already_wrong]

[each_argument]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=%23%5Bderive%28Clone%29%5D%0Astruct+Person+%7B%0A++++_name%3A+String%2C%0A++++friends%3A+Vec%3CPerson%3E%2C%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person+%7B%0A++++++++Person+%7B+_name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26mut+self%2C+other%3A+Person%29+%7B%0A++++++++self.friends.push%28other%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+alice+%3D+Person%3A%3Anew%28%22Alice%22%29%3B%0A++++let+mut+bob+%3D+Person%3A%3Anew%28%22Bob%22%29%3B%0A++++alice.add_friend%28bob.clone%28%29%29%3B%0A++++bob.add_friend%28alice.clone%28%29%29%3B%0A%7D

[^move_semantics]: Again I'm assuming that you've already seen move semantics
    in Rust. If not, here's [the relevant chapter of The
    Book](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html#variables-and-data-interacting-with-move),
    and here's [a comparison with move semantics in
    C++](https://www.youtube.com/watch?v=IPmRDS0OSxM&t=3020).

[^already_wrong]: In fact this example is already out of sync. The copy of Bob
    in Alice's friends list [doesn't get updated] by the second call to
    `add_friend`. A [naive C++ translation] has the same problem.

[doesn't get updated]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=%23%5Bderive%28Clone%29%5D%0Astruct+Person+%7B%0A++++_name%3A+String%2C%0A++++friends%3A+Vec%3CPerson%3E%2C%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person+%7B%0A++++++++Person+%7B+_name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26mut+self%2C+other%3A+Person%29+%7B%0A++++++++self.friends.push%28other%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+alice+%3D+Person%3A%3Anew%28%22Alice%22%29%3B%0A++++let+mut+bob+%3D+Person%3A%3Anew%28%22Bob%22%29%3B%0A++++alice.add_friend%28bob.clone%28%29%29%3B%0A++++bob.add_friend%28alice.clone%28%29%29%3B%0A++++%0A++++%2F%2F+This+assertion+fails.+These+two+Bobs+aren%27t+the+same%21%0A++++assert_eq%21%28bob.friends.len%28%29%2C+alice.friends%5B0%5D.friends.len%28%29%29%3B%0A%7D

[naive C++ translation]: <https://godbolt.org/#g:!((g:!((g:!((h:codeEditor,i:(filename:'1',fontScale:14,fontUsePx:'0',j:1,lang:c%2B%2B,selection:(endColumn:2,endLineNumber:28,positionColumn:2,positionLineNumber:28,selectionStartColumn:2,selectionStartLineNumber:28,startColumn:2,startLineNumber:28),source:'%23include+%3Ccassert%3E%0A%23include+%3Cstring%3E%0A%23include+%3Cvector%3E%0A%0Aclass+Person+%7B%0Apublic:%0A++Person(std::string+name)+:+name(name)+%7B%7D%0A%0A++void+add_friend(Person+other)+%7B%0A++++friends.push_back(other)%3B%0A++%7D%0A%0A++std::string+name%3B%0A++std::vector%3CPerson%3E+friends%3B%0A%7D%3B%0A%0Aint+main()+%7B%0A++Person+alice(%22Alice%22)%3B%0A++Person+bob(%22Bob%22)%3B%0A%0A++//+These+are+copies,+which+allocate+their+own+%60name%60+strings+and+%60friends%60+vectors.%0A++alice.add_friend(bob)%3B%0A++bob.add_friend(alice)%3B%0A%0A++//+The+copy+of+Bob+in+Alice!'s+friends+list+was+not+updated+by+add_friend+above.%0A++assert(bob.friends.size()+%3D%3D+1)%3B%0A++assert(alice.friends%5B0%5D.friends.size()+%3D%3D+0)%3B%0A%7D'),l:'5',n:'0',o:'C%2B%2B+source+%231',t:'0')),k:50,l:'4',m:48.281033364974206,n:'0',o:'',s:0,t:'0'),(g:!((h:executor,i:(argsPanelShown:'1',compilationPanelShown:'0',compiler:g132,compilerName:'',compilerOutShown:'0',execArgs:'',execStdin:'',fontScale:14,fontUsePx:'0',j:2,lang:c%2B%2B,libs:!(),options:'',overrides:!((name:edition,value:'2021')),runtimeTools:!(),source:1,stdinPanelShown:'1',wrap:'1'),l:'5',n:'0',o:'Executor+x86-64+gcc+13.2+(C%2B%2B,+Editor+%231)',t:'0')),k:50,l:'4',n:'0',o:'',s:0,t:'0')),l:'2',n:'0',o:'',t:'0')),version:4>

Like most garbage-collected languages, Python doesn't have this problem,
because it passes objects around "by reference". Can we use references in Rust?

## Part Two: Borrowing

No we can't, because Rust doesn't let us mutate objects that are
borrowed.[^interior_mutability] If we [use shared references]:

[^interior_mutability]: The exception to this rule is "interior mutability",
    and we'll get to that in the next section.

[use shared references]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=struct+Person%3C%27friends%3E+%7B%0A++++name%3A+String%2C%0A++++friends%3A+Vec%3C%26%27friends+Person%3C%27friends%3E%3E%2C%0A%7D%0A%0Aimpl%3C%27friends%3E+Person%3C%27friends%3E+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person%3C%27friends%3E+%7B%0A++++++++Person+%7B+name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26mut+self%2C+other%3A+%26%27friends+Person%3C%27friends%3E%29+%7B%0A++++++++self.friends.push%28other%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+alice+%3D+Person%3A%3Anew%28%22Alice%22%29%3B%0A++++let+mut+bob+%3D+Person%3A%3Anew%28%22Bob%22%29%3B%0A++++alice.add_friend%28%26bob%29%3B%0A++++bob.add_friend%28%26alice%29%3B%0A%7D

```rust
alice.add_friend(&bob);
bob.add_friend(&alice);
```

We get a compiler error when we try to modify Bob:

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

If we [use mutable references], we can avoid aliasing Bob by going through
Alice's friends list to modify him:[^many_to_many]

[use mutable references]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=struct+Person%3C%27friends%3E+%7B%0A++++name%3A+String%2C%0A++++friends%3A+Vec%3C%26%27friends+mut+Person%3C%27friends%3E%3E%2C%0A%7D%0A%0Aimpl%3C%27friends%3E+Person%3C%27friends%3E+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person%3C%27friends%3E+%7B%0A++++++++Person+%7B+name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26mut+self%2C+other%3A+%26%27friends+mut+Person%3C%27friends%3E%29+%7B%0A++++++++self.friends.push%28other%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+alice+%3D+Person%3A%3Anew%28%22Alice%22%29%3B%0A++++let+mut+bob+%3D+Person%3A%3Anew%28%22Bob%22%29%3B%0A++++alice.add_friend%28%26mut+bob%29%3B%0A++++alice.friends%5B0%5D.add_friend%28%26mut+alice%29%3B%0A%7D%0A

[^many_to_many]: This is worth a shot, but the uniqueness rule means we can't
    use mutable references for many-to-many relationships, so we definitely
    can't make object soup out of them in general.

```rust
alice.add_friend(&mut bob);
alice.friends[0].add_friend(&mut alice);
```

But we still get an error about aliasing Alice:

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
them work.[^party_trick] Object soup wants to do aliasing and mutation at the
same time, and that's exactly what references in Rust are supposed to prevent.
We need something different.

[^party_trick]: Ok I lied. We can get something working [by combining shared
    references and interior mutability][party_trick]. Circular borrows in safe
    code! It's a neat party trick, but it's not useful in real programs,
    because it [breaks if we try to move anything][breaks].

[party_trick]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Acell%3A%3ARefCell%3B%0A%0Astruct+Person%3C%27friends%3E+%7B%0A++++_name%3A+String%2C%0A++++friends%3A+RefCell%3CVec%3C%26%27friends+Person%3C%27friends%3E%3E%3E%2C%0A%7D%0A%0Aimpl%3C%27friends%3E+Person%3C%27friends%3E+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person%3C%27friends%3E+%7B%0A++++++++Person+%7B+_name%3A+name.into%28%29%2C+friends%3A+RefCell%3A%3Anew%28Vec%3A%3Anew%28%29%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26self%2C+other%3A+%26%27friends+Person%3C%27friends%3E%29+%7B%0A++++++++self.friends.borrow_mut%28%29.push%28other%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+alice+%3D+Person%3A%3Anew%28%22Alice%22%29%3B%0A++++let+bob+%3D+Person%3A%3Anew%28%22Bob%22%29%3B%0A++++alice.add_friend%28%26bob%29%3B%0A++++bob.add_friend%28%26alice%29%3B%0A%7D

[breaks]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Acell%3A%3ARefCell%3B%0A%0Astruct+Person%3C%27friends%3E+%7B%0A++++name%3A+String%2C%0A++++friends%3A+RefCell%3CVec%3C%26%27friends+Person%3C%27friends%3E%3E%3E%2C%0A%7D%0A%0Aimpl%3C%27friends%3E+Person%3C%27friends%3E+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person%3C%27friends%3E+%7B%0A++++++++Person+%7B+name%3A+name.into%28%29%2C+friends%3A+RefCell%3A%3Anew%28Vec%3A%3Anew%28%29%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26self%2C+other%3A+%26%27friends+Person%3C%27friends%3E%29+%7B%0A++++++++self.friends.borrow_mut%28%29.push%28other%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+alice+%3D+Person%3A%3Anew%28%22Alice%22%29%3B%0A++++let+bob+%3D+Person%3A%3Anew%28%22Bob%22%29%3B%0A++++alice.add_friend%28%26bob%29%3B%0A++++bob.add_friend%28%26alice%29%3B%0A++++vec%21%5Balice%2C+bob%5D%3B%0A%7D

[^advice]: It's worth spending some time "fighting the borrow checker" to build
    up intuition about what works and what doesn't. But when you get stuck, a
    good rule of thumb is to avoid putting lifetime parameters on structs.

## Part Three: Interior Mutability

If you search for "how to mutate a shared object in Rust", you'll find articles
about `Rc`[^rc] and `RefCell`,[^refcell] but **`Rc<RefCell<T>>` doesn't work
well for object soup.** To see why, [let's try it]:

[^rc]: [`Rc`](https://doc.rust-lang.org/std/rc/struct.Rc.html) stands for
    ["reference counting"](https://en.wikipedia.org/wiki/Reference_counting),
    which is the strategy it uses to free its contents. It behaves like a
    shared reference with no lifetime. It's similar to `std::shared_ptr` in C++
    and automatic reference counting in Swift.

[^refcell]: [`RefCell`](https://doc.rust-lang.org/std/cell/struct.RefCell.html)
    is like an
    [`RwLock`](https://doc.rust-lang.org/stable/std/sync/struct.RwLock.html)
    that panics instead of blocking and can't be shared across threads. It lets
    us get `&mut T` from `&RefCell<T>` (which we get from `Rc`).

[let's try it]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Acell%3A%3ARefCell%3B%0Ause+std%3A%3Arc%3A%3ARc%3B%0A%0Astruct+Person+%7B%0A++++_name%3A+String%2C%0A++++friends%3A+Vec%3CRc%3CRefCell%3CPerson%3E%3E%3E%2C%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person+%7B%0A++++++++Person+%7B+_name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26mut+self%2C+other%3A+Rc%3CRefCell%3CPerson%3E%3E%29+%7B%0A++++++++self.friends.push%28other%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+alice+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Alice%22%29%29%29%3B%0A++++let+bob+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Bob%22%29%29%29%3B%0A++++alice.borrow_mut%28%29.add_friend%28Rc%3A%3Aclone%28%26bob%29%29%3B%0A++++bob.borrow_mut%28%29.add_friend%28Rc%3A%3Aclone%28%26alice%29%29%3B%0A%0A++++%2F%2F+Rc%3A%3Aclone+creates+a+new+Rc+pointing+to+the+same+shared+object.+Any%0A++++%2F%2F+changes+we+make+to+the+original+are+visible+through+the+new+Rc.%0A++++let+bob_ref+%3D+%26alice.borrow%28%29.friends%5B0%5D%3B%0A++++assert_eq%21%28bob_ref.borrow%28%29.friends.len%28%29%2C+1%29%3B%0A%7D

```rust
let alice = Rc::new(RefCell::new(Person::new("Alice")));
let bob = Rc::new(RefCell::new(Person::new("Bob")));
alice.borrow_mut().add_friend(Rc::clone(&bob));
bob.borrow_mut().add_friend(Rc::clone(&alice));
```

There's a lot going on there,[^a_lot_going_on] and it's pretty verbose, but it
compiles and runs. That's progress! Unfortunately it has a memory
leak,[^difficult_to_leak] which we can see if we [run it under ASan] or
Miri.[^miri] To fix that, we need to either [explicitly break cycles] before
Alice and Bob go out of scope or [use `Weak` references][weak]. Both options are
error-prone.[^asymmetrical]

[^a_lot_going_on]: `borrow_mut` returns a [smart
    pointer](https://doc.rust-lang.org/book/ch15-00-smart-pointers.html) type
    called [`RefMut`](https://doc.rust-lang.org/std/cell/struct.RefMut.html)
    that implements the
    [`Deref`](https://doc.rust-lang.org/std/ops/trait.Deref.html) and
    [`DerefMut`](https://doc.rust-lang.org/std/ops/trait.DerefMut.html) traits.
    A lot of Rust magic works through those traits and ["deref
    coercions"](https://doc.rust-lang.org/book/ch15-02-deref.html). [Spelling
    out all the types] is helpful for seeing what's going on. The same pattern
    comes up with `Arc<Mutex<T>>`, which is fundamental for multithreading.

[Spelling out all the types]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Acell%3A%3A%7BRefCell%2C+RefMut%7D%3B%0Ause+std%3A%3Aops%3A%3A%7BDeref%2C+DerefMut%7D%3B%0Ause+std%3A%3Arc%3A%3ARc%3B%0A%0Astruct+Person+%7B%0A++++_name%3A+String%2C%0A++++friends%3A+Vec%3CRc%3CRefCell%3CPerson%3E%3E%3E%2C%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person+%7B%0A++++++++Person+%7B+_name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26mut+self%2C+other%3A+Rc%3CRefCell%3CPerson%3E%3E%29+%7B%0A++++++++self.friends.push%28other%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+alice+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Alice%22%29%29%29%3B%0A++++let+bob+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Bob%22%29%29%29%3B%0A%0A++++%2F%2F+alice.borrow_mut%28%29.add_friend%28Rc%3A%3Aclone%28%26bob%29%29%3B%0A++++let+alice_refcell%3A++++%26RefCell%3CPerson%3E++++%3D+alice.deref%28%29%3B%0A++++let+mut+alice_refmut%3A+RefMut%3CPerson%3E++++++%3D+alice_refcell.borrow_mut%28%29%3B%0A++++let+alice_mut%3A++++++++%26mut+Person+++++++++%3D+alice_refmut.deref_mut%28%29%3B%0A++++let+bob_alias%3A++++++++Rc%3CRefCell%3CPerson%3E%3E+%3D+Rc%3A%3Aclone%28%26bob%29%3B%0A++++alice_mut.add_friend%28bob_alias%29%3B%0A++++drop%28alice_refmut%29%3B%0A%0A++++%2F%2F+bob.borrow_mut%28%29.add_friend%28Rc%3A%3Aclone%28%26alice%29%29%3B%0A++++let+bob_refcell%3A++++%26RefCell%3CPerson%3E++++%3D+bob.deref%28%29%3B%0A++++let+mut+bob_refmut%3A+RefMut%3CPerson%3E++++++%3D+bob_refcell.borrow_mut%28%29%3B%0A++++let+bob_mut%3A++++++++%26mut+Person+++++++++%3D+bob_refmut.deref_mut%28%29%3B%0A++++let+alice_alias%3A++++Rc%3CRefCell%3CPerson%3E%3E+%3D+Rc%3A%3Aclone%28%26alice%29%3B%0A++++bob_mut.add_friend%28alice_alias%29%3B%0A++++drop%28bob_refmut%29%3B%0A%7D

[^difficult_to_leak]: Usually it's hard to leak memory by accident in Rust, but
    reference cycles in `Rc` and `Arc` are the main exception. Again this is
    [similar to C++] and to Swift, and you can make Python do the same thing if
    you call
    [`gc.disable`](https://docs.python.org/3/library/gc.html#gc.disable).

[similar to C++]: <https://godbolt.org/#g:!((g:!((g:!((h:codeEditor,i:(filename:'1',fontScale:14,fontUsePx:'0',j:1,lang:c%2B%2B,selection:(endColumn:2,endLineNumber:28,positionColumn:2,positionLineNumber:28,selectionStartColumn:2,selectionStartLineNumber:28,startColumn:2,startLineNumber:28),source:'%23include+%3Ccassert%3E%0A%23include+%3Cmemory%3E%0A%23include+%3Cstring%3E%0A%23include+%3Cvector%3E%0A%0Aclass+Person+%7B%0Apublic:%0A++Person(std::string+name)+:+name(name)+%7B%7D%0A%0A++void+add_friend(std::shared_ptr%3CPerson%3E+other)+%7B%0A++++friends.push_back(std::move(other))%3B%0A++%7D%0A%0A++std::string+name%3B%0A++std::vector%3Cstd::shared_ptr%3CPerson%3E%3E+friends%3B%0A%7D%3B%0A%0Aint+main()+%7B%0A++auto+alice+%3D+std::make_shared%3CPerson%3E(%22Alice%22)%3B%0A++auto+bob+%3D+std::make_shared%3CPerson%3E(%22Bob%22)%3B%0A%0A++//+Passing+by+shared_ptr+increments+the+reference+count+without+copying+the+Person.%0A++alice-%3Eadd_friend(bob)%3B%0A++bob-%3Eadd_friend(alice)%3B%0A%0A++//+Unfortunately+this+reference+cycle+is+a+memory+leak+if+we+don!'t+break+it.%0A++//+alice-%3Efriends.clear()%3B%0A%7D'),l:'5',n:'1',o:'C%2B%2B+source+%231',t:'0')),k:50,l:'4',m:48.281033364974206,n:'0',o:'',s:0,t:'0'),(g:!((h:executor,i:(argsPanelShown:'1',compilationPanelShown:'0',compiler:g132,compilerName:'',compilerOutShown:'0',execArgs:'',execStdin:'',fontScale:14,fontUsePx:'0',j:2,lang:c%2B%2B,libs:!(),options:'-fsanitize%3Daddress',overrides:!((name:edition,value:'2021')),runtimeTools:!(),source:1,stdinPanelShown:'1',wrap:'1'),l:'5',n:'0',o:'Executor+x86-64+gcc+13.2+(C%2B%2B,+Editor+%231)',t:'0')),k:50,l:'4',n:'0',o:'',s:0,t:'0')),l:'2',n:'0',o:'',t:'0')),version:4>

[run it under ASan]: <https://godbolt.org/#g:!((g:!((g:!((h:codeEditor,i:(filename:'1',fontScale:14,fontUsePx:'0',j:1,lang:rust,selection:(endColumn:2,endLineNumber:24,positionColumn:2,positionLineNumber:24,selectionStartColumn:2,selectionStartLineNumber:24,startColumn:2,startLineNumber:24),source:'use+std::cell::RefCell%3B%0Ause+std::rc::Rc%3B%0A%0Astruct+Person+%7B%0A++++_name:+String,%0A++++friends:+Vec%3CRc%3CRefCell%3CPerson%3E%3E%3E,%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new(name:+%26str)+-%3E+Person+%7B%0A++++++++Person+%7B+_name:+name.into(),+friends:+Vec::new()+%7D%0A++++%7D%0A%0A++++fn+add_friend(%26mut+self,+other:+Rc%3CRefCell%3CPerson%3E%3E)+%7B%0A++++++++self.friends.push(other)%3B%0A++++%7D%0A%7D%0A%0Afn+main()+%7B%0A++++let+alice+%3D+Rc::new(RefCell::new(Person::new(%22Alice%22)))%3B%0A++++let+bob+%3D+Rc::new(RefCell::new(Person::new(%22Bob%22)))%3B%0A++++alice.borrow_mut().add_friend(Rc::clone(%26bob))%3B%0A++++bob.borrow_mut().add_friend(Rc::clone(%26alice))%3B%0A%7D'),l:'5',n:'0',o:'Rust+source+%231',t:'0')),k:50,l:'4',m:48.281033364974206,n:'0',o:'',s:0,t:'0'),(g:!((h:executor,i:(argsPanelShown:'1',compilationPanelShown:'0',compiler:nightly,compilerName:'',compilerOutShown:'0',execArgs:'',execStdin:'',fontScale:14,fontUsePx:'0',j:2,lang:rust,libs:!(),options:'-Zsanitizer%3Daddress',overrides:!((name:edition,value:'2021')),runtimeTools:!(),source:1,stdinPanelShown:'1',wrap:'1'),l:'5',n:'0',o:'Executor+rustc+nightly+(Rust,+Editor+%231)',t:'0')),k:50,l:'4',n:'0',o:'',s:0,t:'0')),l:'2',n:'0',o:'',t:'0')),version:4>

[explicitly break cycles]: <https://godbolt.org/#g:!((g:!((g:!((h:codeEditor,i:(filename:'1',fontScale:14,fontUsePx:'0',j:1,lang:rust,selection:(endColumn:2,endLineNumber:27,positionColumn:2,positionLineNumber:27,selectionStartColumn:2,selectionStartLineNumber:27,startColumn:2,startLineNumber:27),source:'use+std::cell::RefCell%3B%0Ause+std::rc::Rc%3B%0A%0Astruct+Person+%7B%0A++++_name:+String,%0A++++friends:+Vec%3CRc%3CRefCell%3CPerson%3E%3E%3E,%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new(name:+%26str)+-%3E+Person+%7B%0A++++++++Person+%7B+_name:+name.into(),+friends:+Vec::new()+%7D%0A++++%7D%0A%0A++++fn+add_friend(%26mut+self,+other:+Rc%3CRefCell%3CPerson%3E%3E)+%7B%0A++++++++self.friends.push(other)%3B%0A++++%7D%0A%7D%0A%0Afn+main()+%7B%0A++++let+alice+%3D+Rc::new(RefCell::new(Person::new(%22Alice%22)))%3B%0A++++let+bob+%3D+Rc::new(RefCell::new(Person::new(%22Bob%22)))%3B%0A++++alice.borrow_mut().add_friend(Rc::clone(%26bob))%3B%0A++++bob.borrow_mut().add_friend(Rc::clone(%26alice))%3B%0A%0A++++//+Break+the+reference+cycle.%0A++++alice.borrow_mut().friends.clear()%3B%0A%7D'),l:'5',n:'0',o:'Rust+source+%231',t:'0')),k:50,l:'4',m:48.281033364974206,n:'0',o:'',s:0,t:'0'),(g:!((h:executor,i:(argsPanelShown:'1',compilationPanelShown:'0',compiler:nightly,compilerName:'',compilerOutShown:'0',execArgs:'',execStdin:'',fontScale:14,fontUsePx:'0',j:2,lang:rust,libs:!(),options:'-Zsanitizer%3Daddress',overrides:!((name:edition,value:'2021')),runtimeTools:!(),source:1,stdinPanelShown:'1',wrap:'1'),l:'5',n:'0',o:'Executor+rustc+nightly+(Rust,+Editor+%231)',t:'0')),k:50,l:'4',n:'0',o:'',s:0,t:'0')),l:'2',n:'0',o:'',t:'0')),version:4>

[weak]: <https://godbolt.org/#g:!((g:!((g:!((h:codeEditor,i:(filename:'1',fontScale:14,fontUsePx:'0',j:1,lang:rust,selection:(endColumn:2,endLineNumber:24,positionColumn:2,positionLineNumber:24,selectionStartColumn:2,selectionStartLineNumber:24,startColumn:2,startLineNumber:24),source:'use+std::cell::RefCell%3B%0Ause+std::rc::%7BRc,+Weak%7D%3B%0A%0Astruct+Person+%7B%0A++++_name:+String,%0A++++friends:+Vec%3CWeak%3CRefCell%3CPerson%3E%3E%3E,%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new(name:+%26str)+-%3E+Person+%7B%0A++++++++Person+%7B+_name:+name.into(),+friends:+Vec::new()+%7D%0A++++%7D%0A%0A++++fn+add_friend(%26mut+self,+other:+%26Rc%3CRefCell%3CPerson%3E%3E)+%7B%0A++++++++self.friends.push(Rc::downgrade(other))%3B%0A++++%7D%0A%7D%0A%0Afn+main()+%7B%0A++++let+alice+%3D+Rc::new(RefCell::new(Person::new(%22Alice%22)))%3B%0A++++let+bob+%3D+Rc::new(RefCell::new(Person::new(%22Bob%22)))%3B%0A++++alice.borrow_mut().add_friend(%26bob)%3B%0A++++bob.borrow_mut().add_friend(%26alice)%3B%0A%7D'),l:'5',n:'0',o:'Rust+source+%231',t:'0')),k:50,l:'4',m:48.281033364974206,n:'0',o:'',s:0,t:'0'),(g:!((h:executor,i:(argsPanelShown:'1',compilationPanelShown:'0',compiler:nightly,compilerName:'',compilerOutShown:'0',execArgs:'',execStdin:'',fontScale:14,fontUsePx:'0',j:2,lang:rust,libs:!(),options:'-Zsanitizer%3Daddress',overrides:!((name:edition,value:'2021')),runtimeTools:!(),source:1,stdinPanelShown:'1',wrap:'1'),l:'5',n:'0',o:'Executor+rustc+nightly+(Rust,+Editor+%231)',t:'0')),k:50,l:'4',n:'0',o:'',s:0,t:'0')),l:'2',n:'0',o:'',t:'0')),version:4>

[^miri]: Tools → Miri [on the Playground][tools]

[tools]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Acell%3A%3ARefCell%3B%0Ause+std%3A%3Arc%3A%3ARc%3B%0A%0Astruct+Person+%7B%0A++++_name%3A+String%2C%0A++++friends%3A+Vec%3CRc%3CRefCell%3CPerson%3E%3E%3E%2C%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person+%7B%0A++++++++Person+%7B+_name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26mut+self%2C+other%3A+Rc%3CRefCell%3CPerson%3E%3E%29+%7B%0A++++++++self.friends.push%28other%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+alice+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Alice%22%29%29%29%3B%0A++++let+bob+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Bob%22%29%29%29%3B%0A++++alice.borrow_mut%28%29.add_friend%28Rc%3A%3Aclone%28%26bob%29%29%3B%0A++++bob.borrow_mut%28%29.add_friend%28Rc%3A%3Aclone%28%26alice%29%29%3B%0A%7D

[^asymmetrical]: `Weak` references are a good fit for asymmetrical
    relationships like child nodes and parent nodes in a tree, but here it's
    not clear who should be weak and who should be strong. If all friends are
    weak, then we need to hold strong references somewhere else to keep people
    alive.

As our program grows, the uniqueness rule will also come back to bite us in the
form of `RefCell` panics. To provoke that, let's change `add_friend` to check
for people befriending themselves. Here's [the change in
Python][friend_self_python]:[^same_name]

[friend_self_python]: <https://godbolt.org/#g:!((g:!((g:!((h:codeEditor,i:(filename:'1',fontScale:14,fontUsePx:'0',j:1,lang:python,selection:(endColumn:22,endLineNumber:13,positionColumn:22,positionLineNumber:13,selectionStartColumn:22,selectionStartLineNumber:13,startColumn:22,startLineNumber:13),source:'class+Person:%0A++++def+__init__(self,+name):%0A++++++++self.name+%3D+name%0A++++++++self.friends+%3D+%5B%5D%0A%0A++++def+add_friend(self,+other):%0A++++++++if+other.name+!!%3D+self.name:%0A++++++++++++self.friends.append(other)%0A%0Aalice+%3D+Person(%22Alice%22)%0Abob+%3D+Person(%22Bob%22)%0Aalice.add_friend(bob)%0Abob.add_friend(alice)'),l:'5',n:'0',o:'Python+source+%231',t:'0')),k:50,l:'4',m:48.281033364974206,n:'0',o:'',s:0,t:'0'),(g:!((h:executor,i:(argsPanelShown:'1',compilationPanelShown:'0',compiler:python311,compilerName:'',compilerOutShown:'0',execArgs:'',execStdin:'',fontScale:14,fontUsePx:'0',j:1,lang:python,libs:!(),options:'',overrides:!(),runtimeTools:!(),source:1,stdinPanelShown:'1',wrap:'1'),l:'5',n:'0',o:'Executor+Python+3.11+(Python,+Editor+%231)',t:'0')),header:(),k:50,l:'4',n:'0',o:'',s:0,t:'0')),l:'2',n:'0',o:'',t:'0')),version:4>

[^same_name]: No two people ever have the same name. It's fine.

```python
def add_friend(self, other):
    if other.name != self.name:
        self.friends.append(other)
```

And [in Rust][friend_self_rust]:

[friend_self_rust]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Acell%3A%3ARefCell%3B%0Ause+std%3A%3Arc%3A%3ARc%3B%0A%0Astruct+Person+%7B%0A++++name%3A+String%2C%0A++++friends%3A+Vec%3CRc%3CRefCell%3CPerson%3E%3E%3E%2C%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person+%7B%0A++++++++Person+%7B+name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26mut+self%2C+other%3A+%26Rc%3CRefCell%3CPerson%3E%3E%29+%7B%0A++++++++if+other.borrow%28%29.name+%21%3D+self.name+%7B%0A++++++++++++self.friends.push%28Rc%3A%3Aclone%28other%29%29%3B%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+alice+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Alice%22%29%29%29%3B%0A++++let+bob+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Bob%22%29%29%29%3B%0A++++alice.borrow_mut%28%29.add_friend%28%26bob%29%3B%0A++++bob.borrow_mut%28%29.add_friend%28%26alice%29%3B%0A%0A++++%2F%2F+Break+the+reference+cycle.%0A++++alice.borrow_mut%28%29.friends.clear%28%29%3B%0A%7D

```rust
fn add_friend(&mut self, other: &Rc<RefCell<Person>>) {
    if other.borrow().name != self.name {
        self.friends.push(Rc::clone(other));
    }
}
```

The Rust version compiles, but if we [make Alice call `add_friend` on
herself][on_herself], it panics:

[on_herself]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Acell%3A%3ARefCell%3B%0Ause+std%3A%3Arc%3A%3ARc%3B%0A%0Astruct+Person+%7B%0A++++name%3A+String%2C%0A++++friends%3A+Vec%3CRc%3CRefCell%3CPerson%3E%3E%3E%2C%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person+%7B%0A++++++++Person+%7B+name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26mut+self%2C+other%3A+%26Rc%3CRefCell%3CPerson%3E%3E%29+%7B%0A++++++++if+other.borrow%28%29.name+%21%3D+self.name+%7B%0A++++++++++++self.friends.push%28Rc%3A%3Aclone%28other%29%29%3B%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+alice+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Alice%22%29%29%29%3B%0A++++let+bob+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Bob%22%29%29%29%3B%0A++++alice.borrow_mut%28%29.add_friend%28%26bob%29%3B%0A++++bob.borrow_mut%28%29.add_friend%28%26alice%29%3B%0A++++alice.borrow_mut%28%29.add_friend%28%26alice%29%3B++%2F%2F+panic%3A+already+mutably+borrowed%0A%7D

```
thread 'main' panicked at 'already mutably borrowed: BorrowError',
src/main.rs:15:18
```

The problem is that we "locked" the `RefCell` to get `&mut self`, and that
conflicts with `other.borrow()` when `other` is aliasing `self`.[^deadlock] The
fix is to [avoid `&mut self` methods][avoid] and keep our borrows short-lived,
but this is also error-prone. We might've missed this bug without a test case.

[^deadlock]: In multithreaded code using `Arc<Mutex<T>>` or `Arc<RwLock<T>>`,
    [the same mistake is a deadlock].

[the same mistake is a deadlock]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Async%3A%3A%7BArc%2C+RwLock%7D%3B%0A%0Astruct+Person+%7B%0A++++name%3A+String%2C%0A++++friends%3A+Vec%3CArc%3CRwLock%3CPerson%3E%3E%3E%2C%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person+%7B%0A++++++++Person+%7B+name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26mut+self%2C+other%3A+%26Arc%3CRwLock%3CPerson%3E%3E%29+%7B%0A++++++++if+other.read%28%29.unwrap%28%29.name+%21%3D+self.name+%7B%0A++++++++++++self.friends.push%28Arc%3A%3Aclone%28other%29%29%3B%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+alice+%3D+Arc%3A%3Anew%28RwLock%3A%3Anew%28Person%3A%3Anew%28%22Alice%22%29%29%29%3B%0A++++let+bob+%3D+Arc%3A%3Anew%28RwLock%3A%3Anew%28Person%3A%3Anew%28%22Bob%22%29%29%29%3B%0A++++alice.write%28%29.unwrap%28%29.add_friend%28%26bob%29%3B%0A++++bob.write%28%29.unwrap%28%29.add_friend%28%26alice%29%3B%0A%0A++++println%21%28%22This+is+going+to+deadlock...%22%29%3B%0A++++alice.write%28%29.unwrap%28%29.add_friend%28%26alice%29%3B%0A++++println%21%28%22We+never+make+it+here%21%22%29%3B%0A%7D

[avoid]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Acell%3A%3ARefCell%3B%0Ause+std%3A%3Arc%3A%3ARc%3B%0A%0Astruct+Person+%7B%0A++++name%3A+String%2C%0A++++friends%3A+Vec%3CRc%3CRefCell%3CPerson%3E%3E%3E%2C%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person+%7B%0A++++++++Person+%7B+name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%7D%0A%0Afn+add_friend%28this%3A+%26Rc%3CRefCell%3CPerson%3E%3E%2C+other%3A+%26Rc%3CRefCell%3CPerson%3E%3E%29+%7B%0A++++if+other.borrow%28%29.name+%21%3D+this.borrow%28%29.name+%7B%0A++++++++this.borrow_mut%28%29.friends.push%28Rc%3A%3Aclone%28other%29%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+alice+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Alice%22%29%29%29%3B%0A++++let+bob+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Bob%22%29%29%29%3B%0A++++add_friend%28%26alice%2C+%26bob%29%3B%0A++++add_friend%28%26bob%2C+%26alice%29%3B%0A++++add_friend%28%26alice%2C+%26alice%29%3B%0A%0A++++%2F%2F+Break+the+reference+cycle.%0A++++alice.borrow_mut%28%29.friends.clear%28%29%3B%0A%7D

`Rc<RefCell<T>>` isn't a good way to write object soup, because it has problems
with aliasing and cycles.[^unsafe_code] Again we need something different.

[^unsafe_code]: Unsafe code has similar problems. Unless you're extremely
    careful, raw pointer soup usually breaks the uniqueness rule when you
    convert pointers back into references to call safe functions. That's
    undefined behavior in Rust, [even when the same code would've been legal in
    C or C++](https://www.youtube.com/watch?v=DG-VLezRkYQ).

## Part Four: Indexes

We can do better with simpler tools. Keep Alice and Bob in a `Vec` and have
them [refer to each other by index]:

[refer to each other by index]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=struct+Person+%7B%0A++++name%3A+String%2C%0A++++friends%3A+Vec%3Cusize%3E%2C%0A%7D%0A%0Afn+new_person%28people%3A+%26mut+Vec%3CPerson%3E%2C+name%3A+%26str%29+-%3E+usize+%7B%0A++++people.push%28Person+%7B+name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%29%3B%0A++++people.len%28%29+-+1%0A%7D%0A%0Afn+add_friend%28people%3A+%26mut+Vec%3CPerson%3E%2C+this_id%3A+usize%2C+other_id%3A+usize%29+%7B%0A++++if+people%5Bother_id%5D.name+%21%3D+people%5Bthis_id%5D.name+%7B%0A++++++++people%5Bthis_id%5D.friends.push%28other_id%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+people+%3D+Vec%3A%3Anew%28%29%3B%0A++++let+alice_id+%3D+new_person%28%26mut+people%2C+%22Alice%22%29%3B%0A++++let+bob_id+%3D+new_person%28%26mut+people%2C+%22Bob%22%29%3B%0A++++add_friend%28%26mut+people%2C+alice_id%2C+bob_id%29%3B%0A++++add_friend%28%26mut+people%2C+bob_id%2C+alice_id%29%3B%0A++++add_friend%28%26mut+people%2C+alice_id%2C+alice_id%29%3B+%2F%2F+no-op%0A%7D

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


This is how we write object soup in Rust. We still need to avoid `&mut self`
methods, and each function has an extra `people` argument. But [aliasing
mistakes] are compiler errors instead of panics, and there's [no risk of memory
leaks]. We can also [serialize the `Vec` with `serde`][serde][^serialize_rc] or
[parallelize it with `rayon`][rayon].

[aliasing mistakes]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=struct+Person+%7B%0A++++name%3A+String%2C%0A++++friends%3A+Vec%3Cusize%3E%2C%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new%28people%3A+%26mut+Vec%3CPerson%3E%2C+name%3A+%26str%29+-%3E+usize+%7B%0A++++++++people.push%28Person+%7B+name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%29%3B%0A++++++++people.len%28%29+-+1%0A++++%7D%0A%0A++++%2F%2F+MISTAKE%3A+%60%26mut+self%60+methods+don%27t+work+well+in+code+like+this%2C+because%0A++++%2F%2F+we+need+to+refer+to+%60self%60+by+index+to+avoid+mutable+aliasing.+I%0A++++%2F%2F+should%27ve+made+this+a+standalone+function+instead.%0A++++fn+add_friend%28%26mut+self%2C+people%3A+%26Vec%3CPerson%3E%2C+other_id%3A+usize%29+%7B%0A++++++++if+people%5Bother_id%5D.name+%21%3D+self.name+%7B%0A++++++++++++self.friends.push%28other_id%29%3B%0A++++++++%7D%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+people+%3D+Vec%3A%3Anew%28%29%3B%0A++++let+alice_id+%3D+Person%3A%3Anew%28%26mut+people%2C+%22Alice%22%29%3B%0A++++let+bob_id+%3D+Person%3A%3Anew%28%26mut+people%2C+%22Bob%22%29%3B%0A++++%2F%2F+These+lines+don%27t+compile%2C+because+we%27re+trying+to+borrow+the+entire+Vec%0A++++%2F%2F+while+we%27re+mutating+one+of+its+elements.%0A++++people%5Balice_id%5D.add_friend%28%26people%2C+bob_id%29%3B%0A++++people%5Bbob_id%5D.add_friend%28%26people%2C+alice_id%29%3B%0A++++people%5Balice_id%5D.add_friend%28%26people%2C+alice_id%29%3B%0A%7D

[no risk of memory leaks]: <https://godbolt.org/#g:!((g:!((g:!((h:codeEditor,i:(filename:'1',fontScale:14,fontUsePx:'0',j:1,lang:rust,selection:(endColumn:2,endLineNumber:24,positionColumn:2,positionLineNumber:24,selectionStartColumn:2,selectionStartLineNumber:24,startColumn:2,startLineNumber:24),source:'struct+Person+%7B%0A++++name:+String,%0A++++friends:+Vec%3Cusize%3E,%0A%7D%0A%0Afn+new_person(people:+%26mut+Vec%3CPerson%3E,+name:+%26str)+-%3E+usize+%7B%0A++++people.push(Person+%7B+name:+name.into(),+friends:+Vec::new()+%7D)%3B%0A++++people.len()+-+1%0A%7D%0A%0Afn+add_friend(people:+%26mut+Vec%3CPerson%3E,+this_id:+usize,+other_id:+usize)+%7B%0A++++if+people%5Bother_id%5D.name+!!%3D+people%5Bthis_id%5D.name+%7B%0A++++++++people%5Bthis_id%5D.friends.push(other_id)%3B%0A++++%7D%0A%7D%0A%0Afn+main()+%7B%0A++++let+mut+people+%3D+Vec::new()%3B%0A++++let+alice_id+%3D+new_person(%26mut+people,+%22Alice%22)%3B%0A++++let+bob_id+%3D+new_person(%26mut+people,+%22Bob%22)%3B%0A++++add_friend(%26mut+people,+alice_id,+bob_id)%3B%0A++++add_friend(%26mut+people,+bob_id,+alice_id)%3B%0A++++add_friend(%26mut+people,+alice_id,+alice_id)%3B+//+no-op%0A%7D'),l:'5',n:'0',o:'Rust+source+%231',t:'0')),k:50,l:'4',m:48.281033364974206,n:'0',o:'',s:0,t:'0'),(g:!((h:executor,i:(argsPanelShown:'1',compilationPanelShown:'0',compiler:nightly,compilerName:'',compilerOutShown:'0',execArgs:'',execStdin:'',fontScale:14,fontUsePx:'0',j:2,lang:rust,libs:!(),options:'-Zsanitizer%3Daddress',overrides:!((name:edition,value:'2021')),runtimeTools:!(),source:1,stdinPanelShown:'1',wrap:'1'),l:'5',n:'0',o:'Executor+rustc+nightly+(Rust,+Editor+%231)',t:'0')),k:50,l:'4',n:'0',o:'',s:0,t:'0')),l:'2',n:'0',o:'',t:'0')),version:4>

[serde]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+serde%3A%3ASerialize%3B%0A%0A%23%5Bderive%28Serialize%29%5D%0Astruct+Person+%7B%0A++++name%3A+String%2C%0A++++friends%3A+Vec%3Cusize%3E%2C%0A%7D%0A%0Afn+new_person%28people%3A+%26mut+Vec%3CPerson%3E%2C+name%3A+%26str%29+-%3E+usize+%7B%0A++++people.push%28Person+%7B+name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%29%3B%0A++++people.len%28%29+-+1%0A%7D%0A%0Afn+add_friend%28people%3A+%26mut+Vec%3CPerson%3E%2C+this_id%3A+usize%2C+other_id%3A+usize%29+%7B%0A++++if+people%5Bother_id%5D.name+%21%3D+people%5Bthis_id%5D.name+%7B%0A++++++++people%5Bthis_id%5D.friends.push%28other_id%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+-%3E+anyhow%3A%3AResult%3C%28%29%3E+%7B%0A++++let+mut+people+%3D+Vec%3A%3Anew%28%29%3B%0A++++let+alice_id+%3D+new_person%28%26mut+people%2C+%22Alice%22%29%3B%0A++++let+bob_id+%3D+new_person%28%26mut+people%2C+%22Bob%22%29%3B%0A++++add_friend%28%26mut+people%2C+alice_id%2C+bob_id%29%3B%0A++++add_friend%28%26mut+people%2C+bob_id%2C+alice_id%29%3B%0A++++add_friend%28%26mut+people%2C+alice_id%2C+alice_id%29%3B+%2F%2F+no-op%0A%0A++++%2F%2F+Serialize+the+people+Vec+into+a+JSON+string.%0A++++let+json+%3D+serde_json%3A%3Ato_string_pretty%28%26people%29%3F%3B%0A++++println%21%28%22%7B%7D%22%2C+json%29%3B%0A%0A++++Ok%28%28%29%29%0A%7D

[^serialize_rc]: `Rc` implements `Serialize` if you [enable the `rc`
    feature](https://serde.rs/feature-flags.html#rc), but trying to serialize a
    reference cycle will [trigger infinite recursion and panic].

[trigger infinite recursion and panic]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+std%3A%3Acell%3A%3ARefCell%3B%0Ause+std%3A%3Arc%3A%3ARc%3B%0Ause+serde%3A%3ASerialize%3B%0A%0A%23%5Bderive%28Serialize%29%5D%0Astruct+Person+%7B%0A++++_name%3A+String%2C%0A++++friends%3A+Vec%3CRc%3CRefCell%3CPerson%3E%3E%3E%2C%0A%7D%0A%0Aimpl+Person+%7B%0A++++fn+new%28name%3A+%26str%29+-%3E+Person+%7B%0A++++++++Person+%7B+_name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%0A++++%7D%0A%0A++++fn+add_friend%28%26mut+self%2C+other%3A+Rc%3CRefCell%3CPerson%3E%3E%29+%7B%0A++++++++self.friends.push%28other%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+alice+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Alice%22%29%29%29%3B%0A++++let+bob+%3D+Rc%3A%3Anew%28RefCell%3A%3Anew%28Person%3A%3Anew%28%22Bob%22%29%29%29%3B%0A++++alice.borrow_mut%28%29.add_friend%28Rc%3A%3Aclone%28%26bob%29%29%3B%0A++++bob.borrow_mut%28%29.add_friend%28Rc%3A%3Aclone%28%26alice%29%29%3B%0A%0A++++serde_json%3A%3Ato_string%28%26alice%29.unwrap%28%29%3B+%2F%2F+panic%3A+stack+overflow%0A%7D

[rayon]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&code=use+rayon%3A%3Aprelude%3A%3A*%3B%0Ause+std%3A%3Athread%3A%3Asleep%3B%0Ause+std%3A%3Atime%3A%3A%7BDuration%2C+Instant%7D%3B%0A%0Astruct+Person+%7B%0A++++name%3A+String%2C%0A++++friends%3A+Vec%3Cusize%3E%2C%0A%7D%0A%0Afn+new_person%28people%3A+%26mut+Vec%3CPerson%3E%2C+name%3A+%26str%29+-%3E+usize+%7B%0A++++people.push%28Person+%7B+name%3A+name.into%28%29%2C+friends%3A+Vec%3A%3Anew%28%29+%7D%29%3B%0A++++people.len%28%29+-+1%0A%7D%0A%0Afn+add_friend%28people%3A+%26mut+Vec%3CPerson%3E%2C+this_id%3A+usize%2C+other_id%3A+usize%29+%7B%0A++++if+people%5Bother_id%5D.name+%21%3D+people%5Bthis_id%5D.name+%7B%0A++++++++people%5Bthis_id%5D.friends.push%28other_id%29%3B%0A++++%7D%0A%7D%0A%0Afn+main%28%29+%7B%0A++++let+mut+people+%3D+Vec%3A%3Anew%28%29%3B%0A++++let+alice_id+%3D+new_person%28%26mut+people%2C+%22Alice%22%29%3B%0A++++let+bob_id+%3D+new_person%28%26mut+people%2C+%22Bob%22%29%3B%0A++++add_friend%28%26mut+people%2C+alice_id%2C+bob_id%29%3B%0A++++add_friend%28%26mut+people%2C+bob_id%2C+alice_id%29%3B%0A++++add_friend%28%26mut+people%2C+alice_id%2C+alice_id%29%3B+%2F%2F+no-op%0A%0A++++%2F%2F+Iterate+over+the+people+Vec+using+multiple+threads.+We+sleep+1+second+for%0A++++%2F%2F+each+person%2C+so+if+this+were+a+single-threaded+loop+it+would+take+2%0A++++%2F%2F+seconds.+But+because+it%27s+multithreaded%2C+it+only+takes+1+second.%0A++++let+start+%3D+Instant%3A%3Anow%28%29%3B%0A++++people.par_iter%28%29.for_each%28%7Cperson%7C+%7B%0A++++++++sleep%28Duration%3A%3Afrom_secs%281%29%29%3B%0A++++++++println%21%28%22%7B%7D%22%2C+person.name%29%3B%0A++++%7D%29%3B%0A++++println%21%28%22Total+time%3A+%7B%3A%3F%7D%22%2C+Instant%3A%3Anow%28%29.duration_since%28start%29%29%3B%0A%7D

## Part Five: Next Steps

Even though we're technically not leaking memory, we can't delete anything from
the `Vec` without messing up the indexes of other elements. One way to allow
for deletion is to replace the `Vec` with a `HashMap`, using either an
incrementing counter or [random UUIDs](https://docs.rs/uuid) for the keys.
There are also more specialized data structures like
[`Slab`](https://docs.rs/slab) and [`SlotMap`](https://docs.rs/slotmap).

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

---

Discussion threads on
[r/rust](https://www.reddit.com/r/rust/comments/17ehphv/object_soup_is_made_of_indexes/?),
[Hacker News](https://news.ycombinator.com/item?id=37983998), and
[lobste.rs](https://lobste.rs/s/zhbv0i/object_soup_is_made_indexes_rust).
