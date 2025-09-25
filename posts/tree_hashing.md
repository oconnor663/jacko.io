# Two Attacks on Naive Tree Hashes
###### 2025 March 30<sup>th</sup>

If you're building a tree hash,[^merkle] and you want it to have the same
security properties as a cryptographic hash like SHA-3 or BLAKE3, there are a
couple of "attacks" you need to know about. Here's a naive recursive tree hash
based on SHA-3 in Python, which turns out to be vulnerable to these
attacks:[^godbolt1][^structure]

[^merkle]: also known as a [Merkle tree]

[Merkle tree]: https://en.wikipedia.org/wiki/Merkle_tree

[^godbolt1]: [Godbolt link][godbolt1]

[godbolt1]: <https://godbolt.org/#z:OYLghAFBqd5QCxAYwPYBMCmBRdBLAF1QCcAaPECAMzwBtMA7AQwFtMQByARg9KtQYEAysib0QXACx8BBAKoBnTAAUAHpwAMvAFYTStJg1AAHAJ4EEA0kvrICeAZUboAwqloBXFgxABmLqTOADJ4DJgAcl4ARpjEIJIAbKTGqAqEDgxunt5%2BASlp9gIhYZEsMXGJ1pi2hQxCBEzEBFlePv5VNRn1jQTFEdGx8UkKDU0tOe0jPX2l5UMAlNaoHsTI7BxUxKgsANQITAoItHhRO3gsKU07h0y%2BAPoATACsCQCkGgCC7x9B2B8AYnchABJABa2B2r18ABEdlwNAjvt8sFQdgRiJhMHd9ocIKFjB4CHcouZMAp5iBvjtqWdUfQGHiGASiSSCGT5pDfC4obDfgCgWDsJTPjTRZCHr4dsCmYSFDsYrRUAB3HZMHZrJpMULXPAAL0wqox4oe9CYqIYGDJrweD1IqoY6B2SoNVExjosmBYVLF1OtkvwGLstFMZ0EqB2QgAEh8ALS%2BAB03p9GIIKwY13292eCUZzOJpPJ8fwwDJBAg8yRIppfp2AHUEIw0Q3Q8yznKWCQDRZDDsBAbTajnddjMcCGcx9r9rQqHbAys0gA3aqmJPiyXozHYg4Ics7TBMZAIPZiGc7bSobUetFK8MKDxRDcGnENuWhIiq43GRqMAirmsWrBrVte1HVdTB3X2CcwwjaM40TKtqSnVEeR2elc0JfM2XJHYAHocJ2B5V3oKgiTvB8MSxZ9OVhR8t1xfEMNZK0ngAIRAJDXieaEKwQnZiDwYAEFI%2B9aKolDRO3dCWQLTiWKQykuJ4j5RS/DFBDuACDRQ4jhPIzcxIeFi%2BIEoS7jIiTDlXFM0wzW5HheCBVJ/DTLXmIsBNLctKy%2BCUdkjWJMDADg5R7TBVFYEcDQYgg7QIG8%2BMMdBtmDHYSzCYgmDZR1TSXBR4I%2Bc5LjHJRkBTBRvmirhqOuTBSswAg8qIABrRhMLJCAHh2AAqHY%2BUBEFwSU58qvEii6J3SqlKRXyPgdPYAqCkKLQ9YgW0JeM60CjhaFodVLFSLsEDwOU%2BzRcMYibA00AYUQ2WYLLpslZ8yV7VErxyl7UFRSq7TweNMA2sKDwIFKrwUVgtJtJzBB2TSgKbTLHrRMbnykrgORvDxaEdYwtnQDw1nygdxpGmFbKzBzKtkkA%2BoFcFOO49ySxGLzPmJ59OpQm4KZzKnWNpgahQZtzi08pTos5sn2e3EbDNQ/cqHGwjPg56qLImmUCAeKbPhrAAVF8u3i6KQqNNUgLQHbjoyeGBGR/Tt3yg4lCuSqdjAMAUIlkDj0OUnoRQjnvJrfyMUW%2B1UBWtaYqdI7D1VdB0BC9VlkinYOyNDKHW2eWmFys7o64fKawAeVRNB5wNIdRAYIKx2ukZiAJqDmUlY4WqbY7aSdA0ktrjgxyai0VUq/KM6xJi5S52qyvjZrWsnjrut6v5%2BsFcXNclL3Ndlozx7a8qVe3LeyfVqTfB1nzJRYwke/VHs0AuQkn2Pp1CEsW%2Bh%2BVUJgELu0SWNGFO6%2BAjDw2GsXXyBsu5d3NjaekwALB7lUHdNIdtMoNGQE1ICTsFAuzHM%2BE%2BAcybc3sjmYa4ojJn33pPeYIsPIs0vrjN8EAgIfB2pdVUuDYgNR2F%2BLh6APZAXmBwRYtBOBPF4D4DgWhSCoE4Mocwlh0wKGWKsSG/heAEE0CIxYTUQBPA0PoTgkhJHaNkZwXgCgQCGK0dIkRpA4CwCQGFWqhISDkEoI0YAChlCGGqEISwSopE8FII/YwdBMoZF8WEWgATlRSJkWEuggwzAWAEP4XwoTtjhPoMQcIEMLFZIuMk4gJdCRxKCWYlxyAPjEG8YU6p9QQGFP4IIEQYh2BSBkIIRQKh1B2NILoAIBgjAgFSUo/QJwrGQEWKgYwtQrEcEsaotYegRggOif4wJwTeBKgysYTgPBRHiNMQMuRHBsCqFcUQVaCi0npgTFwSUEA7kTJ2LgQgJA1zo00do2hpAGxMCwHEcspA9EGKMRwExpAEm8HOZY6xpBbFaH%2BWIjgDxTkyPhUiv5iwlzEFQT4SQQA>

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
resistance[^assuming] and doesn't allow length extension:[^godbolt2]

[^prefix_suffix]: Prefixing is marginally better for security, because a
    collision on one node type doesn't automatically lead to collisions on
    other types. (See also HMAC-MD5, which is still [technically unbroken].) On
    the other hand, suffixing lets us start hashing the first leaf even if we
    don't know whether it's the root, and our implementation doesn't need a
    leaf-size buffer. A real tree hash might prefix the leaf/parent
    distinguisher but suffix the root/non-root distinguisher.

[technically unbroken]: https://crypto.stackexchange.com/questions/9336/is-hmac-md5-considered-secure-for-authenticating-encrypted-data

[^assuming]: as long as SHA-3 remains collision resistant

[^godbolt2]: [Godbolt link][godbolt2]

[godbolt2]: <https://godbolt.org/#z:OYLghAFBqd5QCxAYwPYBMCmBRdBLAF1QCcAaPECAMzwBtMA7AQwFtMQByARg9KtQYEAysib0QXACx8BBAKoBnTAAUAHpwAMvAFYTStJg1AAHAJ4EEA0kvrICeAZUboAwqloBXFgxAA2AKykzgAyeAyYAHJeAEaYxCCSAOykxqgKhA4Mbp7efoGp6fYCoeFRLLHxSdaYtkUMQgRMxATZXj4B1bWZDU0EJZExcQnJCo3Nrbkdo739ZRXDAJTWqB7EyOwcVMSoLADUCEwKCLR40bt4LKnNu0dMAMwA%2BgBM/r4ApBoAgh%2BfwdifADEHkIAJIALWwuzedwAIrsuBpET8flgqLsGBhMA8DkcIBisA9ouZMApSOcFA9tqgCGS8BTjE1GAQFiAfrt2VCnncof4AEIaMkaN7%2BGEgdECAC0VIIu3oTCobI5by5PP5ZK4wtF4oYUtQ1N2DOITMV7OV3OFvK4gs1Yulssw8pNnPNfKt8JtuzthuNXw5Nw8VBoqihsN2RIIJIgFrplL1NPJD29gk1CydRoIqwYNwOjxevjxmMJxIUnN5/sDeFUCwAdPhgCSCBBU19kSq5AwTgBrTC7Cw9mjEUa7ADiGGi7hlJwYnbJFjpuyDmHQ50u9DYgiYdX2h12aFoJ3SAlb3KN6WmDHWu0My/QqBJDDAHBlYloqAA7vajBZdphVBGGIeDDViimBogQRpYjiCAQGExgeAQRYRqSCbStCMIACrEB4mAsk6eBovQDAwQwcEIeGJILCGLhobsfyAsC4LYKyvp%2Bhy6aZuKBJQcRpGISStIUtKAmJoyyawgCYhKM2nx%2BgctBojRhE8fBfEKJRAD06m7E8Tr0FQCEKB40TgZgkGHAgIZwiZZm4rBKnkQoFogHJCoisJqHiZJOFpngwAIAZRnWdi5mWb2EHBbZJH2cWFouaybkoXGaESbQUlOkmCH4j2imgQFxnhVBpaer5/kPIZ%2BWmRFCBppgGbEFmWVVRAGUPFl7lxsJGVoZh2HSceuwABJxJgj4loYP6qKwxj0OcUXxgQb6oJ6147LQpi7PW4TEJuS72kwABuJLAV8FxXDKSjIOmjknXNXChRdV3VkQ3YMKpEBPLsABUtH/ECoIQtJUF3TRQXcXZBBcH1LYqp8DDLggw2jVeGJ9sQs2kdWuwAOojRw%2B67pYaQ9nOJYCMTS2xL2CO7gIoj/jt6D9VBJK7KgYHU3Kh2k2i4Nung1aYJjv5MHYa1Uz2CisNlTxPBlnGYMqH0WJu/Wg%2BZykQ5Ri0eLQy7GNs6AeOsx2fHKVBVcDoa3LmrwaxqfIgHRf2MSmta%2BQ2TY/GbVUfTR1vPLbvMWk7DEQvFMI1nWHvSeDvuht7QNFQn5k6V8UFx1ZBXq7HUPfCq6EI0ovaLej8FjUaX2fREADy6Gfd9TCck8e4Hpkius1matHCbhxKNcidgGANHp8i0PckNRpI4Y1II2j4Nkm%2BCB4MgFlMOg6BjTTHjTT2LAkD221wzse1c72S28ybZq7NXaJoKsRdvj2ogPk%2BNMAeBRsyuD3JdsTS8lvhXYj9di3hfjKTsGIPwXx%2BHvI0ql7qYEurVBQT1UAvTeh9b6Id/rYBjnNc0oZeZFVgViByPwoIEMzpVMG%2BDc5XwLtLJ4hFgDfl/P%2BQCV4CCNGQJ2dut4Wb1xrnXb6i1iCdipvOewbAe4KD7jKChuxB5%2BxzAHfMidlRli7tBEhqkFiR3dqMT2LYvj6zCI2RWptaDiyvLIuIBASwMlsegJRMsFgcCWLQTg/heA%2BA4FoUgqBODKHMJYLMCgVhrGlncHgpACCaHcUsTsIB/ACk8RwSQPj4kBM4LwBQIABRxL8e40gcBYBIF/Ig%2BCJByCUCaMABQyhDA1CEJYN8viYloEuHQTcmRGnhFoC098vj/GdOMHQIYZgLACGiXcUgozxnEAiFLHJcydhjPoMQau8FBltKyRU5AnxiD1JWfsho%2BBfG8H4IIEQYh2BSBkIIRQKh1BFNILoK0BgjAgEmaE/Qpw8mQCWKgYwdQ8kcFyRE9YehRjnL6c01p7TeBvm2sYTgPAPFeMya8wJHBsCqEqUQNGwSplZjuNWLg3IIDEt%2BbsXAhASDOkhrwQpWg9GkARmvIYTZSBJJSfoTgGTSDDN4Di3J%2BTYnxLZWkp4WL/GiolUUtlh1ByZASEAA%3D%3D>

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
