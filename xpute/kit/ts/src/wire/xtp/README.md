# XTP: Xpute Tree Packet

XTP is the kit's tree packet: a tree of values laid in one buffer and read in
place, child by child, with no decode up front. It is a file's format: that of
the data a program stores and serves. The kit implements it in TypeScript
(here), Rust (`kit/rust/src/wire/xtp/`) and C++ (`cpp/xpute/kit/wire/xtp/`),
and the golden records under `kit/spec/golden/wire/xtp/` hold the three to the
same bytes. What crosses the link is not XTP but frames, their packets the
Canonical ABI's; an XTP packet a method carries is bytes to the link.

XTP gives a tree its shape and its values' types, and no meaning: which child
is which field, what order the fields come in and what a reader of an older
packet does are a layer above it. So a plain object has no form of its own; a
program lowers it to a tuple in the order its schema owns (`to_tuple`,
`from_tuple` in `lower.ts`).

## The model

A tree has three kinds of node. A number or a `bool` is a **scalar leaf**. A
typed array, a string, a list of strings or a set of bits is a **sequence
leaf**, kept packed. A branch holds its children by index.

```text
  [42, "hello", true, Float64Array [1.1, 2.2, 3.3]]

  Branch
  ├─ Scalar(i32)     = 42
  ├─ Sequence(str)   = "hello"
  ├─ Scalar(bool)    = true
  └─ Sequence(f64[]) = [1.1, 2.2, 3.3]
```

A branch names each child by its offset from the branch itself, so a subtree
is relocatable: an encoded packet can be grafted whole into another tree
(`graft`), and a reader reaches any child directly, reading only what it asks
for, one level (`get`) or the whole subtree (`get_deep`).

Absence is of two kinds. `NIL` is an untyped null node. A typed node may be
absent too, keeping its type while its payload is not there (`str(null)`,
`u32(null)`, `branch(null)`): a reader learns what the node would have been,
and `opt()` answers `undefined` for it.

## Lowering JavaScript values

The TypeScript writes a JavaScript value by one rule: `null` is `NIL`, a
`number` an `i32` where it fits and an `f64` otherwise, a `bigint` an `i64`, a
`boolean` a `bool`, a `string` a `str`, a typed array its packed sequence, and
an array a branch. The split of a number is this language's policy, not the
wire's; a caller that wants another type writes it by its own writer (`u8`,
`u32`, `f32`, …). A value outside the rule, or a `bigint` outside `i64`, is a
programmer's error and throws.

## The packet

This is the wire contract: any implementation that follows it reads and
writes the same packets. Every number is little-endian, and a word is 8 bytes.

```text
packet
├─ header (16 bytes)
│  ├─ word0: [magic 32 | reserved 32]
│  └─ word1: [root_payload_sz 32 | type 8 | reserved 24]
└─ root payload
```

`magic` is the bytes `XTP\0`, and reserved bits are zero. A root payload
starts at the header's end; `root_payload_sz` 0 is an absent root, whose type
is kept. A packet's length is padded to a whole word, and is at most
2^31 − 1 bytes.

```text
  bit      7     6     5     4  3    2  1  0
        ┌─────┬─────┬─────┬────────┬─────────┐
        │  L  │  R  │  S  │   C    │    A    │
        └─────┴─────┴─────┴────────┴─────────┘
```

In the type byte, `L` set is a leaf, and clear is `NIL` (`0x00`) or `BRANCH`
(`0x20`); `R` is reserved and zero; `S` set is a sequence, clear a scalar. `C`
is 0 unsigned, 1 signed, 2 float and 3 not a number. `A` is, for a number,
log2 of its width in bytes, and otherwise 0 `bool` (or `bitset` as a
sequence), 1 `str` and 2 `strs`. So a `u32` is `0x82`, an `f32` array `0xb2`
and a `str` `0xb9`. No other byte is a type.

A scalar is its payload alone, aligned to its own width; a `bool` is one byte,
0 or 1.

A sequence begins on a word with a len slot, one word `[len 32 | reserved
32]`, then its payload, padded to a word. An array's payload is its elements.
A `bitset`'s `len` counts bits, packed eight to a byte, the first in the
lowest bit. A `str`'s `len` counts its UTF-8 bytes, and its payload carries one
trailing 0 byte past them. A `strs`'s `len` counts its strings, and its
payload is `len + 1` u32 offsets, never decreasing, then the UTF-8 they index:
string `i` lies between offsets `i` and `i + 1`.

```text
branch node
├─ len slot
└─ child table
   ├─ [child_rel_off | child_type]
   ├─ [child_rel_off | child_type]
   └─ ...
```

A branch begins and ends on a word, and each entry of its table is one word.
Its children follow the table in order, each aligned as its type requires,
each offset from the branch's start. `child_rel_off` 0 is an absent child,
whose type is kept, and `NIL` is an absent node whose type is `NIL`. A present
child starts past the table and before the next present sibling, and ends by
that sibling's start or its parent's end, so no two nodes share bytes.

## Errors

The TypeScript throws where a packet breaks the contract: an encoder on a
malformed graft or a packet past its size, and a reader on a malformed header,
payload bound, child offset or leaf, or a read past the tree. A graft is
checked only for its alignment as it is added, and whole as the tree is
encoded. The API itself is in the code's doc comments (`view.ts`,
`encode.ts`, `cursor.ts`).
