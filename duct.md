# Announcing duct

Duct is a library for running child processes and building pipelines. Two
libraries, in fact, one in Python and one in Rust. Its goal is to colonize more
languages and gradually help people stop writing important software in Bash.

Python already has [lots](https://amoffat.github.io/sh/) and
[lots](https://plumbum.readthedocs.io/en/latest/) and
[lots](https://github.com/kennethreitz/envoy) of libraries like this, so why
one more? Duct wants to do a few things differently:

- *Use an API that's easy to port.* The duct API fits in any language with
  fluent method calls. There's no magic, and certainly no string concatenation.
- *Run any pipeline that Bash can.* Duct expressions are trees of objects,
  and that lets us support wacky things like `(a && b) | (c && d)`.
- *Fail fast.* Any non-zero exit status in any child process is an error by
  default. This is similar to `set -e -o pipefail` in Bash.
