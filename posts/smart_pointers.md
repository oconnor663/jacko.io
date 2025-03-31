# Smart Pointers Can't Solve Use-After-Free
###### 2025 February 24<sup>th</sup>

A common question: "If we use smart pointers everywhere, can C++ be as 'safe'
as [Circle] or Rust?"

[Circle]: https://safecpp.org/draft.html

There are several reasons the answer is no, but the immediate reason is that
you can't use smart pointers everywhere, because there are internal raw
pointers in types you don't control.[^want] For example, here's an iterator
invalidation mistake with `std::vector`:

[^want]: For a language that uses smart pointers everywhere automatically, see
    [Swift].

[Swift]: https://docs.swift.org/swift-book/documentation/the-swift-programming-language/automaticreferencecounting/

```c++
std::vector<int> my_vector = {1, 2, 3};
for (auto element : my_vector) {
    if (element == 2) {
        my_vector.push_back(4);
        // The next loop iteration reads a dangling pointer.
    }
}
```

```
LINK: Godbolt https://godbolt.org/z/Gnvc96zrK
==1==ERROR: AddressSanitizer: heap-use-after-free on address 0x502000000018
READ of size 4 at 0x502000000018 thread T0
```

This fails ASan with a heap-use-after-free error,[^click] because `vector`
iterators are raw pointers.[^reallocate] Putting each `int` in a `shared_ptr`,
or putting the `vector` itself in a `shared_ptr`, [doesn't help].[^doesnt_help]

[^click]: Click the "Godbolt" button to see it run.

[^reallocate]: A `vector` holds all its elements in an array on the heap. When
    we call `push_back` here, it notices that the array doesn't have any empty
    slots. So it allocates a bigger array, copies all the elements over, and
    frees the old array. The problem is that the `begin` and `end` iterators
    created by the `for` loop are still pointing to the old array.

[doesn't help]: https://godbolt.org/z/83zbz377r

[^doesnt_help]: A `shared_ptr<int>` can't become dangling, but a
    `shared_ptr<int>*` (i.e. a pointer _to_ a `shared_ptr<int>`) can. It's
    similar to how an `int**` becomes dangling when you destroy the `int*` that
    it points to, even if the `int` is still alive.

You can make the same mistake with `std::span` (C++20):

```c++
std::vector<int> my_vector{1, 2, 3};
std::span<int> my_span = my_vector;
my_vector.push_back(4);
// This line reads a dangling pointer.
int first = my_span[0];
```

```
LINK: Godbolt https://godbolt.org/z/rs9G1qETe
==1==ERROR: AddressSanitizer: heap-use-after-free on address 0x502000000010
READ of size 4 at 0x502000000010 thread T0
```

You can even make the same mistake with `std::lock_guard` (C++11):[^tsan]

[^tsan]: TSan and Valgrind both catch this one, but ASan doesn't. [ASan doesn't
    instrument `pthread_mutex_unlock`][lobsters_comment], but TSan replaces it,
    and I suppose Valgrind instruments everything at runtime.

[lobsters_comment]: https://lobste.rs/s/e8cnqe/smart_pointers_can_t_solve_use_after_free#c_4ktple

```c++
std::shared_ptr<std::mutex> my_mutex = std::make_shared<std::mutex>();
std::lock_guard my_guard(*my_mutex);
my_mutex.reset();
// my_guard calls my_mutex->unlock() in its destructor.
```

```
LINK: Godbolt https://godbolt.org/z/a7q46jvad
WARNING: ThreadSanitizer: heap-use-after-free (pid=1)
  Atomic read of size 1 at 0x721000000010 by main thread
```

This sort of thing is why [`std2::lock_guard`][lock_guard] in Circle and
[`std::sync::MutexGuard`][mutex_guard] in Rust both have "lifetime
annotations".

[lock_guard]: https://github.com/cppalliance/safe-cpp/blob/889685274438ca20344d4d9cb472e4392c4e35a9/libsafecxx/single-header/std2.h#L1235
[mutex_guard]: https://doc.rust-lang.org/std/sync/struct.MutexGuard.html
