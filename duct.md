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
  and that lets us represent wacky things like `(a && b) | (c && d) 1>&2`.
- **Fail fast.** Any non-zero exit status in any child process is an error by
  default. This is similar to `set -e -o pipefail` in Bash.

## What's wrong with Bash?

First things first, there's a lot that's right with Bash. For programs that
spend most of their time shelling out, Bash syntax is perfect. It supports
hilariously flexible pipelines, usually in a single line of code. It has a
cross-platform install base that Perl and Python dream about. And as the de
facto standard Unix shell, it's pretty much guaranteed to stay that way.

But Bash makes it hard to write reliable code. Whitespace splitting
[burns](http://unix.stackexchange.com/q/131766/23305) new programmers
until they learn to quote everything. Simple string and path operations
tend to be [buggy
shortcuts](https://bugs.chromium.org/p/chromium/issues/detail?id=660145)
for lack of libraries. And [error
handling](http://www.artima.com/intv/handcuffs2.html) is limited: errors
are either ignored by default, or terminate the entire program with
`set -e`.

None of this is news to Bash programmers, but sometimes there aren't other
options. When you can't install dependencies on the target machine, what are
you going to do? Write native code? ...? Five years ago, before Rust and Go
were kicking around, that was a rhetorical question. Now, maybe it's just a
long shot. Duct aims to make all these long shots a little bit shorter.

## Python Example

```python
# Run a command. This inherits stdin/stdout/sterr from the parent, and
# it throws if the exit status isn't zero.
cmd("git", "log").run()

# Read the standard output of a command. First we do it the long way.
result = cmd("echo", "foo").stdout_capture().run()
assert 0 == result.status
assert b"foo\n" == result.stdout

# Now do the same thing with the `read` convenience method, which
# behaves like shell backticks.
output = cmd("echo", "foo").read()
assert "foo" == output

# Run a string of shell code in the OS shell. This will run under `/bin/sh`
# on Unix and `cmd.exe` on Windows:
sh("cat <<EOF\nHello world!\nEOF").run()

# Set an env var and redirect stdout to a file.
cmd("git", "status").env("GIT_DIR", "/tmp/foo").stdout("/tmp/bar").run()

# Pipe three expressions into a fourth.
echo1 = cmd("echo", "foo")
echo2 = cmd("echo", "bar")
echo3 = cmd("echo", "baz")
grep = sh("grep ba")
echo1.then(echo2).then(echo3).pipe(grep).run()

# Ignore a non-zero exit status.
cmd("false").unchecked().then(sh("echo ignored the error")).run()
```

## Rust Example

```rust
// Run a command. This inherits stdin/stdout/sterr from the parent, and
// returns an error if the exit status isn't zero.
cmd!("git", "log").run()?;

// Read the standard output of a command. First we do it the long way.
let output: std::process::Output = cmd!("echo", "foo").stdout_capture().run()?;
assert!(output.status.success());
assert_eq!(&b"foo\n"[..], &output.stdout[..]);

// Now do the same thing with the `read` convenience method, which
// behaves like shell backticks.
let output: String = cmd!("echo", "foo").read()?;
assert_eq!("foo", output);

// Run a string of shell code in the OS shell. This will run under `/bin/sh`
// on Unix and `cmd.exe` on Windows:
sh("cat <<EOF\nHello world!\nEOF").run()?;

// Set an env var and redirect stdout to a file.
cmd!("git", "status").env("GIT_DIR", "/tmp/foo").stdout("/tmp/bar").run()?;

// Pipe three expressions into a fourth.
let echo1 = cmd!("echo", "foo");
let echo2 = cmd!("echo", "bar");
let echo3 = cmd!("echo", "baz");
let grep = sh("grep ba");
echo1.then(echo2).then(echo3).pipe(grep).run()?;

// Ignore a non-zero exit status.
cmd!("false").unchecked().then(sh("echo ignored the error")).run()?;
```
