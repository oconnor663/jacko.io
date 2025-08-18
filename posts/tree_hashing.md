# Two Attacks on Naive Tree Hashes
###### 2025 March 30<sup>th</sup>

If you're building a tree hash,[^merkle] and you want it to have the same
security properties as a cryptographic hash like SHA-3 or BLAKE3, there are a
couple of "attacks" you need to know about. Here's a naive recursive tree hash
based on SHA-3 in Python, which turns out to be vulnerable to these
attacks:[^structure]

[^merkle]: also known as a [Merkle tree]

[Merkle tree]: https://en.wikipedia.org/wiki/Merkle_tree

[^structure]: Dividing the length by 2 at each level of the tree like this is a
    shortcut, but the resulting tree structure isn't great, because we have to
    know the final input length to figure out where any of the leaf or subtree
    boundaries are. That's no problem in these examples, but in practice most
    hash functions need to be able to ["stream" long inputs in a loop][update]
    without knowing the final length until the end. A better splitting rule for
    real tree hashes is to make the left side as large as possible as long as
    it's a power-of-2. See [the BLAKE3 paper], particularly sections 2.1 and
    5.1.2, and also [Binary Numeral Trees].

[update]: https://docs.python.org/3/library/hashlib.html#hashlib.hash.update
[the BLAKE3 paper]: https://github.com/BLAKE3-team/BLAKE3-specs/blob/master/blake3.pdf
[Binary Numeral Trees]: https://eprint.iacr.org/2021/038.pdf

```python
from hashlib import sha3_256

LEAF_SIZE = 1000

def tree_hash(input_bytes):
    if len(input_bytes) <= LEAF_SIZE:
        # Inputs below a certain size are "leaf nodes", and we feed them
        # directly into SHA-3.
        return sha3_256(input_bytes).digest()

    # When the input is more than one leaf we split it in half, recursively
    # tree_hash() each half, join the two subtree hashes into a "parent
    # node", and feed that into SHA-3.
    half = len(input_bytes) // 2
    left_subtree_hash = tree_hash(input_bytes[:half])
    right_subtree_hash = tree_hash(input_bytes[half:])
    parent_node = left_subtree_hash + right_subtree_hash
    return sha3_256(parent_node).digest()
```

Let's start with arguably the most important security property for
cryptographic hashes, **collision resistance**. A "collision" is when two
different inputs have the same hash. There are no known collisions on
SHA-3.[^collisions] But even though our `tree_hash` uses SHA-3 on the inside,
it has collisions:[^random]

[^collisions]: As of 2025, there are no known collisions on SHA-2 either. The
    first collision on SHA-1 was [published in 2017](https://shattered.io/).

[^random]: I'm going to use [`secrets.token_bytes`] instead of
    [`random.randbytes`] in all these examples. `randbytes` is more familiar,
    but it uses a non-cryptographic RNG called [Mersenne Twister] that has
    [predictable output]. Even when it doesn't really matter (like here), we
    might as well teach the good one.

[`random.randbytes`]: https://docs.python.org/3/library/random.html#random.randbytes
[`secrets.token_bytes`]: https://docs.python.org/3/library/secrets.html#secrets.token_bytes
[Mersenne Twister]: https://en.wikipedia.org/wiki/Mersenne_Twister
[predictable output]: https://github.com/oconnor663/mersenne_breaker

```python
# Here's an example input, two randomly generated leaves.
import secrets
input1 = secrets.token_bytes(2 * LEAF_SIZE)
hash1 = tree_hash(input1)

# And here's another input. We'll choose this one to be the concatenated
# hashes of the leaves of input1, i.e. exactly the same "parent node" that
# tree_hash(input1) would produce.
leaf_hash1 = sha3_256(input1[:LEAF_SIZE]).digest()
leaf_hash2 = sha3_256(input1[LEAF_SIZE:]).digest()
input2 = leaf_hash1 + leaf_hash2
hash2 = tree_hash(input2)

# These two inputs are a "collision" on tree_hash.
assert input1 != input2 and hash1 == hash2
```

SHA-3 also prevents **length extension**.[^extension] In other words, seeing
the hash of some secret input doesn't let us compute any _related_ hashes, like
say the hash of "that secret plus some more bytes". But our `tree_hash` does
allow length extension:[^both_directions]

[^extension]: Surprisingly, MD5, SHA-1, and SHA-2 all allow length extension.
    That's why historically we've needed [HMAC] for keyed hashing.

[HMAC]: https://en.wikipedia.org/wiki/HMAC

[^both_directions]: In fact, `tree_hash` allows length extension in both
    directions.

```python
# Here's another input, which adds a couple more random leaves to input1.
# Of course we can't construct input3 like this if we don't know input1.
more_bytes = secrets.token_bytes(2 * LEAF_SIZE)
input3 = input1 + more_bytes
hash3 = tree_hash(input3)

# But we can compute hash3 without knowing input1, by "extending" hash1.
# This is a "length extension attack".
assert hash3 == sha3_256(hash1 + tree_hash(more_bytes)).digest()
```

In both cases, the problem is that we're giving the same inputs to SHA-3 in
different places. We have collisions because the bytes of a (random) parent
node can be the same as the bytes of a (deliberately chosen) leaf node. And we
have extensions because there's no difference between the (maybe secret) hashes
in the interior of the tree[^cv] and the (maybe public) hash at the root. This
leads us to two rules for secure tree hashes:

[^cv]: In the literature, interior/non-root hashes are often called "chaining
    values".

1. Leaf hashes and parent hashes must never use exactly the same input.
2. Root hashes and non-root hashes must never use exactly the same input.

One way to satisfy these rules is to prefix or suffix[^prefix_suffix] all our
SHA-3 inputs. Here's a modified `tree_hash` that has collision
resistance[^assuming] and doesn't allow length extension:

[^prefix_suffix]: Prefixing is marginally better for security, because a
    collision on one node type doesn't automatically lead to collisions on
    other types. (See also HMAC-MD5, which is still [technically unbroken].) On
    the other hand, suffixing lets us start hashing the first leaf even if we
    don't know whether it's the root, and our implementation doesn't need a
    leaf-size buffer. A real tree hash might prefix the leaf/parent
    distinguisher but suffix the root/non-root distinguisher.

[technically unbroken]: https://crypto.stackexchange.com/questions/9336/is-hmac-md5-considered-secure-for-authenticating-encrypted-data

[^assuming]: as long as SHA-3 remains collision resistant

```python
def node_hash(node_bytes, is_root, is_parent):
    # [0, 0]: non-root leaf
    # [0, 1]: non-root parent
    # [1, 0]: root leaf
    # [1, 1]: root parent
    suffix = bytes([is_root, is_parent])
    return sha3_256(node_bytes + suffix).digest()

def tree_hash(input_bytes, is_root=True):
    if len(input_bytes) <= LEAF_SIZE:
        return node_hash(input_bytes, is_root, is_parent=False)
    half = len(input_bytes) // 2
    left_subtree_hash = tree_hash(input_bytes[:half], is_root=False)
    right_subtree_hash = tree_hash(input_bytes[half:], is_root=False)
    parent_node = left_subtree_hash + right_subtree_hash
    return node_hash(parent_node, is_root, is_parent=True)
```

For the formal definitions and proofs of these rules, see [_Sufficient
conditions for sound tree and sequential hashing modes_][sufficient] by the
Keccak/SHA-3 team. Section 7.5 ("Checking of some real-world tree hash modes")
is especially interesting even if you aren't into the formal stuff.

[sufficient]: https://keccak.team/files/TreeHashing.pdf
