use std::rc::Rc;
use std::cell::RefCell;
use std::iter::{Iterator, IntoIterator};

// Look at this Python-style iteration. I can mutate a list while I'm
// iterating over it! How does this work? The push() method takes a
// *shared* reference, and everything is reference counted.

fn main() {
    let mylist = PythonicList::new();
    mylist.push(1);
    mylist.push(2);
    mylist.push(3);
    for i in &mylist {
        println!("{}", i);
        if *i == 2 {
            mylist.push(4);
        }
    }
}

struct PythonicList<T> {
    vec: Rc<RefCell<Vec<Rc<T>>>>,
}

impl<T> PythonicList<T> {
    fn new() -> PythonicList<T> {
        PythonicList { vec: Rc::new(RefCell::new(Vec::new())) }
    }

    fn push(&self, item: T) {
        self.vec.borrow_mut().push(Rc::new(item));
    }
}

impl<'a, T> IntoIterator for &'a PythonicList<T> {
    type Item = Rc<T>;
    type IntoIter = PythonicIterator<T>;

    fn into_iter(self) -> PythonicIterator<T> {
        PythonicIterator {
            vec: self.vec.clone(),
            index: 0,
        }
    }
}

struct PythonicIterator<T> {
    vec: Rc<RefCell<Vec<Rc<T>>>>,
    index: usize,
}

impl<T> Iterator for PythonicIterator<T> {
    type Item = Rc<T>;

    fn next(&mut self) -> Option<Rc<T>> {
        let ret = self.vec.borrow().get(self.index).cloned();
        self.index += 1;
        ret
    }
}
