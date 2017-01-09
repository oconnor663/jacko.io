# Announcing Duct

Duct is a library for running child processes and building pipelines. Two
libraries in fact, one in Python and one in Rust. Its goal is to colonize more
languages and gradually help people stop writing important software in Bash.

Python already has [lots](https://amoffat.github.io/sh/) and
[lots](https://plumbum.readthedocs.io/en/latest/) and
[lots](https://github.com/kennethreitz/envoy) of libraries like this, so why
one more? Duct aims to do a few things differently:

- **Use an API that's easy to port.** The Duct API fits in any language that
  has methods. There's no magic, and certainly no string concatenation.
- **Run any pipeline that Bash can.** Duct expressions are trees of objects,
  and that helps us support wacky things like `(a && b) | (c && d)`.
- **Fail fast.** Any non-zero exit status in any child process is an error by
  default. This is similar to `set -e -o pipefail` in Bash.

## What's wrong with Bash?

First things first, there's a lot that's right with Bash. It supports
hilariously flexible pipelines, usually in a single line of code, with no
deadlocks. It has the cross-platform install base that Perl and Python dream
about. And as the de facto standard interactive shell, it's pretty much
guaranteed to stay that way.

That install base is why a lot of systems software and packaging hooks are
still written in Bash. But Bash makes it hard to write reliable code.
Whitespace splitting bites new programmers until they learn to religiously
quote their strings. The lack of any kind of library support means that string
and path operations tend to be [buggy
shortcuts](https://bugs.chromium.org/p/chromium/issues/detail?id=660145). And
error handling is limited: errors are either ignored by default, or terminate
the entire program with stricter settings.

None of this is news to Bash programmers, but it's hard to switch to newer
languages, partly because they're just not convenient enough. Hopefully Duct
libraries can chip away at that difference.
