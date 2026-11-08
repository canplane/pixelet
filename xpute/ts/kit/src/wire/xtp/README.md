# XTP: Xpute Tree Packet

XTP is a tree of values in one buffer, read where it lies. Every frame on
xpute's rings that carries data carries it as an XTP packet. This directory is
the TypeScript implementation; `rust/kit/src/wire/xtp/` and
`cpp/xpute/kit/wire/xtp/` implement the same format in Rust and C++, and the
golden records under `spec/golden/wire/xtp/` hold the three to the same bytes.
This README is the format's reference: what a packet holds, and why it is laid
out the way it is.

## One decision

Nothing inside a packet records where the packet is. Every offset is counted
from the start of the branch that holds it, so the same bytes mean the same
thing at any address. A command's packet is written once into a ring's slot and
read there by the other end, in whatever memory holds it. Across a message port
it is copied as plain bytes and written into another slot unchanged, and a
packet encoded earlier is grafted into a larger one without a number inside it
changing.

Three properties follow from that decision. A payload is read where it lies,
with nothing decoded into objects first. A typed array is handed out as a view
straight into the packet. And a reader goes to any child by its index, reading
only the nodes on the way, so a large packet costs only what is read of it.

What XTP leaves out is a schema. A packet says the type of every node and
nothing of what a node means; a higher layer gives the nodes their meaning,
their order and their compatibility. In xpute that layer is a protocol file: its
generated code writes a command's fields as the children of one branch, in
order, and reads them back by their index.

## The packet

A packet is a 16-byte header and one root node, and every word in it is
little-endian. A count, an offset or a size takes an 8-byte slot, with the value
in its low 32 bits; the high 32 bits are reserved and zero, or they hold a type
byte. Written as fields from the lowest bit, with each field's width in bits:

```text
header   word 0   [ MAGIC 32 | reserved 32 ]
         word 1   [ the root's payload size 32 | type 8 | reserved 24 ]

branch   [ child count 32 | reserved 32 ], then a descriptor for each child:
         [ offset from the branch's start 32 | type 8 | reserved 24 ]

scalar   the value alone, aligned to its own width

sequence [ count 32 | reserved 32 ], then the payload, padded to a word
```

The header's first word holds MAGIC, the bytes "XTP\0" read as a little-endian
word. The second holds the size of the root's payload and the root's type. The
root's payload begins right after the header, and a payload size of 0 means that
the root is absent. A packet's length is padded to a whole word, and a packet is
at most 2^31 − 1 bytes.

## The type byte

Each node's type is one byte. With its top bit set, the byte names a leaf, and
its other bits say which leaf:

```text
  bit      7     6     5     4  3    2  1  0
        ┌─────┬─────┬─────┬────────┬─────────┐
        │  L  │  R  │  S  │   C    │    A    │
        └─────┴─────┴─────┴────────┴─────────┘
```

L is set for a leaf, and R is reserved and zero. S is set for a sequence, which
holds many values after a count, and clear for a scalar, which holds one. C is
the class: 0 for an unsigned integer, 1 for a signed one, 2 for a float, and 3
for a leaf that is not a number. A is the argument. For a number, it is the
base-2 logarithm of the number's width in bytes, so 0 to 3 for an integer and 2
or 3 for a float. For a leaf that is not a number, it says which one: 0 for a
bool as a scalar and a bitset as a sequence, 1 for a string, and 2 for a leaf of
many strings. So a u32 is 0x82, an f32 array is 0xb2, and a string is 0xb9.

With its top bit clear, the byte names a special type: NIL, 0x00, the untyped
null, or BRANCH, 0x20. A third, GRAFT, 0x40, exists only while a tree is being
built: a graft is written as the root of the packet it holds, under that root's
type, so it never appears on the wire. A reader refuses every other byte,
whether it has a reserved bit set, an argument its class does not define, or
GRAFT's value.

## Leaves and branches

A scalar leaf is its value alone, aligned to its own width: a u8 or a bool on
any byte, a u16 on two, a u32 or an f32 on four, and a u64 or an f64 on eight. A
bool is one byte, 0 or 1.

A sequence leaf begins on a word, with its count in a slot, and its payload
follows, padded to a word. An array's payload is its elements, little-endian. A
bitset's count is its number of bits, packed eight to a byte, the first in the
lowest bit. A string's count is its length in UTF-8 bytes, and its payload is
those bytes and one NUL, which the count leaves out. Many strings can share one
leaf, so that a string costs no node of its own: the count says how many strings
there are, and the payload holds one more 32-bit offset than there are strings,
each no less than the one before, and then the UTF-8 they index. String `i` lies
between offsets `i` and `i + 1`.

A branch begins on a word, with its child count in a slot, and a table follows
with one descriptor for each child: the child's offset from the branch's own
start, and the child's type. The children come after the table, in order, each
aligned as its type requires. A branch's extent ends on a word, so whatever
follows it starts on the next word, and every language pads it alike.

```text
a branch and its three children; each descriptor is an offset and a type,
and every byte is counted from the branch's start, as the offsets are

            ┌─────────────────────────────────────────┐
  byte   0  │ count = 3          │ reserved           │
  byte   8  │ +32                │ u32                │──────────┐
  byte  16  │ +40                │ f32 array          │────────┐ │
  byte  24  │ +560               │ branch             │──────┐ │ │
            ├─────────────────────────────────────────┤      │ │ │
  byte  32  │ the u32's value    │ padding            │◀─────┼─┼─┘
  byte  40  │ count = 128        │ reserved           │◀─────┼─┘
  byte  48  │ 512 bytes of f32, its payload           │      │
            │ …                                       │      │
  byte 560  │ child C: its count, table, children     │◀─────┘
            └─────────────────────────────────────────┘
```

## Absence and null

XTP tells two kinds of nothing apart. An absent node keeps its type and has no
payload: a root whose payload size is 0, or a child whose offset is 0, which no
present child can have, since every present child lies past its branch's table.
NIL is the untyped null, written as an absent node whose type is NIL. So a field
that holds a string can be absent and still say that it is a string, and a
reader can tell "no string here" from "nothing here".

In TypeScript, a typed writer given null, such as `str(null)`, `u32(null)` or
`branch(null)`, writes an absent node of its own type, and the generic writer
given null writes NIL. A reader reads both as null, and a cursor's `opt()`
returns undefined for both, for optional chaining.

## What a reader refuses

A packet comes from the far side of a boundary, so a reader trusts none of it.
It refuses a packet that breaks the format with EBADMSG: a header without MAGIC,
or a payload size past the packet's end; a reserved bit set, or a type byte that
no node has; a node not aligned as its type requires; a count or an offset that
runs past the packet; and a string whose bytes are not UTF-8 or that lacks its
NUL. It also refuses a child outside its extent. A present child starts past its
branch's table and before the next present sibling, and it runs no further than
that sibling's start or its parent's end; the root's end is the end of its
payload. So no two nodes share bytes, and the nodes a packet holds are bounded
by its size. Asking a branch for a child past its count is EFAULT.

A reader checks only what it reaches. Opening a packet checks its header, and a
node's bytes are checked when a read reaches them.

## Writing a packet

There are two ways to write a packet, and they give the same bytes. The **packet
writer** writes in place, into bytes the caller already holds, with no tree
built first; this is how the generated code writes a command's fields straight
into a ring's slot. It does not fail a call when a node does not fit. It goes on
counting, and finishing the packet then refuses it with EOVERFLOW, so the caller
learns once whether the packet fits. The **tree encoder** walks a tree built in
memory onto the same writer, into a buffer that doubles until the tree fits, up
to a cap.

In TypeScript, a tree is built with `TreeView`, encoded with the encoder, and
read with `TreeReader`:

```ts
import { encoder, TreeReader, TreeView } from "@xpute/kit/wire/xtp/mod.ts";

const tree = new TreeView().branch((b) => b.u8(255).str("hello").f32(0.5));
const packet = encoder.encode(tree, { init_cap: 256 });

const root = new TreeReader(packet).read_branch();
root.at(1).get<string>(); // "hello"
root.get_deep(); // [255, "hello", 0.5]
```

The typed writers, `u8`, `u64`, `f32`, `str` and the rest, write exactly the
type they name. The generic writers, `set` and `put`, choose each node's type
from its value: a number becomes an i32 where it fits and an f64 otherwise, a
bigint an i64, a boolean a bool, a string a string, a typed array the matching
array, an array a branch, and null NIL. That choice for a number is TypeScript's
way of writing one, not a rule of the wire. An object is refused with EINVAL,
because which keys it carries, and in what order, is a schema's question; a
schema that owns its keys turns an object into an array with `to_tuple` and back
with `from_tuple` (`wire/lower.ts`).

A branch can take an encoded packet as a child with `graft`: the packet's root
is copied in as it lies, under the root's type, and none of its offsets changes,
because each is counted from its own branch. `subtree` appends a tree that is
not yet encoded.

## Reading a packet

A **cursor** is a view of one node. On a leaf, `get()` reads the value. On a
branch, `get()` reads the children shallowly, handing a child branch back as its
own cursor, unread, while `get_deep()` reads the whole branch, and `at(i)` goes
to one child. An absent child's cursor reads as null. A typed array is read as a
view straight into the packet, so in TypeScript the packet must start on a word
boundary in its buffer.

Rust's reader mirrors these cursors. C++'s reader hands back views into the
packet, and it allocates only where the caller asks for an owned string or a
deep read, with the caller's allocator. It copies each number out of the packet,
so its packet need not be aligned in memory, though every node must still be
aligned within the packet.
