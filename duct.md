# Announcing Duct

Duct is a library for running child processes and building pipelines. Two
libraries in fact, one in Python and one in Rust. The goal is to colonize more
languages and gradually help people stop writing important software in Bash.

Rust doesn't have many libraries like this yet, but Python already has
[lots](https://amoffat.github.io/sh/) and
[lots](https://plumbum.readthedocs.io/en/latest/) and
[lots](https://github.com/kennethreitz/envoy) of them, so why one more? Duct
aims to do a few things differently:

- **Use an API that's easy to port.** The Duct API fits in any language that
  has methods. There's no magic, and certainly no string concatenation.
- **Run any pipeline that Bash can.** Duct expressions are trees of objects,
  and that lets us do wacky things like `(a && b) | (c && d) 1>&2`.
- **Fail fast.** Any non-zero exit status in any child process is an error by
  default. This is similar to `set -e -o pipefail` in Bash.

## What's wrong with Bash?

First things first, there's a lot that's right with Bash. For programs that
spend most of their time shelling out, Bash syntax is perfect. It supports
hilariously flexible pipelines, usually in a single line of code. It has a
cross-platform install base that Perl and Python dream about. And as the de
facto standard Unix shell, it's pretty much guaranteed to stay that way.

A lot of systems software and packaging hooks are still written in Bash, but
Bash makes it hard to write reliable code. Whitespace splitting
[burns](http://unix.stackexchange.com/q/131766/23305) new programmers until
they learn to quote everything. Simple string and path operations tend to be
[buggy shortcuts](https://bugs.chromium.org/p/chromium/issues/detail?id=660145)
for lack of libraries. And error handling is limited: errors are either ignored
by default, or terminate the entire program with `set -e`.

None of this is news to Bash programmers, but sometimes there just aren't other
options. When you can't install dependencies on the target machine, what are
you going to do? Write native code? ... Five years ago, before Rust and Go were
kicking around, that was a rhetorical question. Now, maybe it's just a long
shot. Duct aims to make these long shots a little bit shorter.
