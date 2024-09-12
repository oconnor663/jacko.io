struct Foo<'lifetime> {
    n: u64,
    n_ref: &'lifetime u64,
    // other fields...
}

fn foo<'lifetime>(n: u64) -> Foo<'lifetime> {
    // Start with a placeholder reference.
    let mut ret = Foo { n, n_ref: &0 };
    // Then, re-point that reference to `n`. Maybe surprisingly, this line is legal.
    ret.n_ref = &ret.n;
    // But now *this* line is illegal.
    ret
}

fn main() {}
