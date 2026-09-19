# XTP: Xpute Tree Packet

A small **relocatable binary tree packet** format, with two implementations held to each other: this directory in TypeScript and `crates/xpute/core/src/wire/xtp/` in Rust. The wire format is the contract between them; the walkthrough below is the TypeScript API, which the Rust mirrors.

It is designed to stay simple at the surface:

- write roots with `set()`
- append branch children inside `branch((b) => ...)` with `put()`
- read shallow with `get()`
- read deep with `get_deep()`
- use explicit writers when you want exact control

You can think of it like this:

> JSON-like usage for scalars, arrays, strings, and typed arrays, stored as a compact binary tree with lazy reads.

This layer is **not** a schema system by itself.
It provides only the binary tree packet layer; higher layers define field meaning, ordering, compatibility, and validation.

XTP stands for **Xpute Tree Packet**. In the rest of this document, it is described simply as a **tree packet**.

## Wire-format notes

- byte order is **little-endian**
- branch child offsets are relative to the **enclosing branch base**
- `str` stores UTF-8 bytes with a trailing NUL on wire, while its stored `len` excludes that terminator
- root presence is encoded through `root_payload_sz`
- branch-child presence is encoded through `child_rel_off`

The default JS-side lowering path is intentionally small:

- `number` -> `i32` if it fits, otherwise `f64`
- `bigint` -> `i64` only
- `boolean` -> `bool`
- `string` -> `str`
- typed arrays -> packed sequence leaves
- JS arrays -> branch nodes
- `null` -> generic `NIL`

---

## Quick Start

### Encode

```ts
import { TreeEncoder, TreeView } from "@xpute/core/wire/xtp/mod.ts";

const tree = new TreeView();
tree.set([42, "hello", true]);

const packet = new TreeEncoder().encode(tree);

console.log(packet.byteLength > 0);
// true
```

### Read

```ts
import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";

const root = new TreeReader(packet).read_branch();
const out = root.get_deep<[number, string, boolean]>();

console.log(out);
// [42, "hello", true]
```

`read_branch()` is a convenience entry for packets whose root is known to be a
branch. If the root kind is not known ahead of time, use `read()` first and
then inspect or cast the returned cursor.

---

## What This Is Good For

This tree packet format is a good fit when you want:

- structured binary packets
- typed arrays to stay packed
- lazy reads instead of full upfront decode
- direct child access by index
- worker, WASM, IPC, and RPC boundaries
- large payloads you do not want to fully decode up front
- a lower-level binary substrate that higher layers can build on

It is best understood as a **binary tree packet layer** — one piece of a larger serialization stack.

---

## Mental Model

This library stores data as a **binary tree**.

- primitive values become **scalar leaves**
- typed arrays become **sequence leaves**
- JS arrays become **branch nodes**

Example input:

```ts
[
  42,
  "hello",
  true,
  new Float64Array([1.1, 2.2, 3.3]),
];
```

Conceptually becomes:

```text
Branch
├─ Scalar(i32)     = 42
├─ Sequence(str)   = "hello"
├─ Scalar(bool)    = true
└─ Sequence(f64[]) = [1.1, 2.2, 3.3]
```

You usually do **not** need to build that shape manually.
Generic lowering does it for you.

---

## Default Lowering Rules

`TreeView.set()` and values written inside `tree.branch((b) => ...)` follow this contract:

- `null` -> `NIL`
- `number` -> `i32` if it fits, otherwise `f64`
- `bigint` -> `i64` only
- `boolean` -> `bool`
- `string` -> `str`
- `TypedArray` / `BigTypedArray` -> matching packed sequence leaf
- `Array` -> branch node

Notes:

- The `number -> i32 | f64` split is a **JS-side lowering policy**, not a wire-level requirement.
- Generic lowering treats unsupported input or out-of-contract generic bigint input as **programmer error** and throws.
- Explicit scalar writers such as `u8()`, `i32()`, `u64()`, `f32()` are **caller-chosen constructors** and normalize through the word cast helpers.
- Generic `set(null)` / `put(null)` are intentionally distinct from typed optional null such as `str(null)` or `u32(null)`.

That gives you a generic path that feels close to normal JS data entry, while still producing a compact binary layout.

---

## Optional Model

XTP distinguishes **generic null** from **typed optional absence**.

### Generic null

```ts
const tree = new TreeView();
tree.set(null);
```

This lowers to `NIL`, which is an untyped null node.

### Typed optional absence

```ts
const tree = new TreeView();
tree.str(null);
tree.u32(null);
tree.branch(null);
```

These preserve the original node kind while making the payload physically absent.

The encoding model is:

- optional means the node kind is preserved while payload is absent
- packet-root physical absence is encoded as `root_payload_sz == 0`
- branch-child physical absence is encoded as `child_rel_off == 0`
- generic `set(null)` / `put(null)` lower to `NIL`
- typed writers preserve their original node kind

On read:

- `NullCursor` preserves type metadata
- `get()` reconstructs `null`
- `opt()` returns `undefined` for optional chaining

Example:

```ts
import { TreeEncoder, TreeReader, TreeView } from "@xpute/core/wire/xtp/mod.ts";

const tree = new TreeView();
tree.branch((b) =>
  b
    .u32(1)
    .str(null)
    .branch(null)
);

const packet = new TreeEncoder().encode(tree);
const root = new TreeReader(packet).read_branch();

console.log(root.at(0).get<number>()); // 1
console.log(root.at(1).get<string | null>()); // null
console.log(root.at(2).opt()); // undefined
```

Here:

- child 0 is a present `u32`
- child 1 is a typed-null `str`
- child 2 is a typed-null `branch`

These are not the same as generic `NIL`.

---

## Use Case 1: Easy Generic Encoding

```ts
import { TreeEncoder, TreeReader, TreeView } from "@xpute/core/wire/xtp/mod.ts";

const data = [
  42,
  "Hello xpute!",
  true,
  new Float64Array([1.1, 2.2, 3.3]),
] as const;

const tree = new TreeView();
tree.set(data);

const packet = new TreeEncoder().encode(tree);
const value = new TreeReader(packet).read_branch().get_deep<typeof data>();

console.log(value[1]); // "Hello xpute!"
console.log(value[3] instanceof Float64Array); // true
```

This is the default mode:

- write familiar JS values
- encode once
- read back lazily or deeply as needed

---

## Use Case 2: Exact Manual Control

When exact binary shape matters, build explicitly.

```ts
import { TreeEncoder, TreeReader, TreeView } from "@xpute/core/wire/xtp/mod.ts";

const tree = new TreeView();

tree.branch((b) =>
  b
    .u8(255)
    .str("Player_1")
    .f32(99.9)
);

const packet = new TreeEncoder().encode(tree);
const b = new TreeReader(packet).read_branch();

const level = b.at(0).get<number>();
const name = b.at(1).get<string>();
const hp = b.at(2).get<number>();

console.log(`[${level}] ${name} (HP: ${hp})`);
```

Use this style when you want:

- exact leaf type selection
- exact sequence type selection
- predictable binary layout
- no ambiguity about what gets encoded

Explicit writers are also the path to use when you want unsigned 64-bit scalar values:

```ts
const tree = new TreeView();
tree.u64(123n);
```

---

## Use Case 3: Typed Optional Fields

```ts
import { TreeEncoder, TreeReader, TreeView } from "@xpute/core/wire/xtp/mod.ts";

const tree = new TreeView();
tree.branch((b) =>
  b
    .u32(7)
    .str(null)
    .u8_array(null)
);

const packet = new TreeEncoder().encode(tree);
const b = new TreeReader(packet).read_branch();

const id = b.at(0).get<number>();
const name = b.at(1).get<string | null>();
const bytes = b.at(2).opt()?.get<Uint8Array>() ?? null;

console.log({ id, name, bytes });
// { id: 7, name: null, bytes: null }
```

This is useful when your higher layer already knows:

- which field is supposed to be `str`
- which field is supposed to be `u8[]`
- that physical absence should still preserve node kind metadata

---

## Use Case 4: Lazy Random Access

This is where the tree layout becomes especially useful.

```ts
import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";

const list = new TreeReader(huge_payload_bytes).read_branch();

console.log(`total items: ${list.len}`);

const target = list.at(123).get<string>(); // assumes list.len > 123
console.log(target);

for (const child of list) {
  if (child.is_branch()) {
    // handle structural items only
  }
}
```

Important point:

- you do **not** need to decode the whole tree into heap objects first
- you can jump directly to a child with `at(idx)`

---

## Use Case 5: Shallow vs Deep Read

XTP read cursors distinguish **shallow** from **deep** materialization.

```ts
import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";

const root = new TreeReader(packet).read_branch();

const shallow = root.get();
// direct children only:
// - leaves become materialized values
// - branch children stay as BranchCursor
// - null children become null

const deep = root.get_deep();
// recursively materialized NodeValue[]
```

For one child:

```ts
const child = root.get(0);
```

`get(idx)` is also shallow:

- leaf child -> materialized value
- branch child -> `BranchCursor`
- null child -> `null`

This makes it possible to keep structure lazy while still reading leaf values directly.

---

## Use Case 6: Compose Subtrees with `subtree()`

A common real-world pattern is to build subtrees separately and then assemble them into one root packet.

```ts
import { encoder, TreeView } from "@xpute/core/wire/xtp/mod.ts";
import { to_tuple } from "@xpute/core/wire/lower.ts";

const FEAT_ROW_KEYS = ["id", "name"] as const;
const COAST_ROW_KEYS = ["id", "ring"] as const;

const feat_rows = [
  { id: 1, name: "road" },
  { id: 2, name: "building" },
];

const coast_rows = [
  { id: 10, ring: new Uint8Array([1, 2, 3]) },
];

const ele = new Int16Array([10, 20, 30, 40]);

const feats_view = new TreeView().branch((b) => {
  for (const row of feat_rows) {
    b.put(to_tuple(row, FEAT_ROW_KEYS));
  }
});

const coasts_view = new TreeView().branch((b) => {
  for (const row of coast_rows) {
    b.put(to_tuple(row, COAST_ROW_KEYS));
  }
});

const packet = encoder.encode(
  new TreeView().branch((b) =>
    b
      .i16_array(ele)
      .subtree(feats_view)
      .subtree(coasts_view)
  ),
);
```

This pattern is useful when:

- one subtree is produced by one stage
- another subtree is produced by another stage
- the final packet layout still wants everything under one root branch

`subtree()` appends an already-built in-memory subtree.
Unlike `graft()`, it does **not** splice an already-encoded packet.

---

## Core API

### `TreeView`

Mutable root-building façade.

```ts
const tree = new TreeView();
```

Generic root write:

```ts
tree.set(value);
```

Explicit root write:

```ts
tree.i32(123);
tree.str("hello");
tree.bool(true);
tree.u32_array(new Uint32Array([1, 2, 3]));
```

Typed optional root write:

```ts
tree.str(null);
tree.u32(null);
tree.branch(null);
```

Explicit branch build at the root:

```ts
tree.branch((b) => {
  b.i32(1).str("x").bool(true);
});
```

Graft a pre-encoded subtree packet:

```ts
tree.graft(encoded_packet);
```

Explicit scalar writers normalize through cast helpers:

- `u8/i8/u16/i16/u32/i32` -> narrowed to the target width
- `u64/i64` -> normalized with 64-bit bigint casts
- `f32` -> `Math.fround`
- `f64` -> identity cast

### `BranchView`

Mutable child-appending façade used during branch construction.
Most users do not instantiate or reference `BranchView` directly.
The normal entry path is `TreeView`, which exposes branch construction through
`tree.branch((b) => ...)`.

```ts
tree.branch((b) => {
  b.put(1);
  b.put("x");
  b.bool(true);
});
```

Replace all children with one tuple-like sequence:

```ts
b.set([1, "x", true]);
```

Inside `tree.branch((b) => ...)`, both `put(...)` and explicit writers such as
`u32(...)`, `str(...)`, `i16_array(...)` append one child and return the same
`BranchView`, so chaining is valid.

Append an already-built subtree:

```ts
b.subtree(other_tree);
```

Append a pre-encoded subtree packet:

```ts
b.graft(encoded_packet);
```

### `TreeEncoder`

Encodes a built tree into an immutable packet.

```ts
const encoder = new TreeEncoder();
const packet = encoder.encode(tree);
```

`encode()` uses throwing semantics on malformed input or packet-cap overflow.

### `TreeReader`

Packet reader and root cursor entry point.

```ts
const reader = new TreeReader(packet);
const root = reader.read();
const branch = reader.read_branch();
```

Construction validates the fixed packet header and throws on malformed packets.

### `NodeCursor`

Lazy view of one node in an encoded packet.

```ts
root.get();
root.get<T>();
root.is_branch();
root.as_branch();
root.opt();
```

`NodeCursor.get()` is shallow:

- leaf cursor -> materialized leaf value
- null cursor -> `null`
- branch cursor -> handled by `BranchCursor` override

### `BranchCursor`

Navigation interface for branch nodes.

```ts
const b = root.as_branch();

b.len;
b.at(0);
b.at_branch(1);
b.get();
b.get(0);
b.get<T>();
b.get<T>(0);
b.get_deep();
b.children();
```

`BranchCursor` is iterable:

```ts
for (const child of b) {
  // ...
}
```

`BranchCursor.get()` works in two modes:

- `b.get()` -> shallow read of the whole branch
- `b.get(idx)` -> shallow read of one child

`BranchCursor.get_deep()` recursively materializes the whole branch as `NodeValue[]`.

---

## Object-Like Data

Plain objects are not auto-lowered in the generic path, because object lowering immediately raises schema questions this layer leaves to a higher one:

- are keys encoded?
- in what order?
- are missing fields allowed?
- what does compatibility mean?

If you want object-like data today, use explicit tuple mapping in a higher layer.
If your higher layer already owns field order, tuples are usually enough.
For quick-and-dirty usage, `Object.entries()` can also be acceptable, but
`to_tuple()` / `from_tuple()` are the recommended schema-owned path.

Example helper use:

```ts
import { from_tuple, to_tuple } from "@xpute/core/wire/lower.ts";
import { TreeEncoder, TreeReader, TreeView } from "@xpute/core/wire/xtp/mod.ts";

const obj = {
  a: 1,
  b: 2,
  c: "three",
};

const keys = ["a", "b", "c"] as const;

const tree = new TreeView();
tree.set(to_tuple(obj, keys));

const packet = new TreeEncoder().encode(tree);

const out_tuple = new TreeReader(packet).read_branch().get_deep<[number, number, string]>();
const out = from_tuple<typeof obj>(out_tuple, keys);

console.log(out);
// { a: 1, b: 2, c: "three" }
```

Quick prototype path:

```ts
const tree = new TreeView();
tree.set(Object.entries(obj));
```

This can be fine for ad-hoc usage, but tuple mapping is usually the better
long-term path once field order is owned by a schema layer.

This is still just a user-level convention that lowers to an array-of-pairs
branch shape. XTP itself does not define object semantics.

> **Note:** A higher-level schema and type-mapping layer can sit on top of this tree packet layer. That layer can handle plain objects, key encoding, and schema evolution while preserving the core packet layer.

---

## Graft

You can splice a previously encoded subtree packet into another tree.

```ts
import { TreeEncoder, TreeView } from "@xpute/core/wire/xtp/mod.ts";

const child = new TreeView();
child.set([1, 2, 3]);

const child_packet = new TreeEncoder().encode(child);

const parent = new TreeView();
parent.graft(child_packet);

const parent_packet = new TreeEncoder().encode(parent);
```

`graft()` expects a packet intended for this tree format, not an arbitrary byte blob.

Important detail:

- at the **view** layer, `graft()` only enforces construction-boundary checks such as `WORD_SZ` alignment
- full packet validation is deferred to **encode/read time**

A grafted root may also be physically absent.
In that case, root type metadata is still preserved.

Conceptually, graft lets you reuse a pre-encoded subtree as part of a larger packet.

---

## Packet Layout

You usually do not need to care about the byte layout first, but it helps later.

### Packet

```text
packet
├─ header (16 bytes)
│  ├─ word0: [magic 32 | reserved 32]
│  └─ word1: [root_payload_sz 32 | type 8 | reserved 24]
└─ root payload
```

Root rules:

- `root_payload_sz == 0` -> physically absent root
- `root_payload_sz > 0` -> present root payload
- present root payload starts exactly at `HDR_SZ`
- root type is preserved even when `root_payload_sz == 0`

### Scalar Leaf

```text
scalar node
└─ payload only
```

### Sequence Leaf

```text
sequence node
├─ len slot
└─ payload
```

For `str` specifically:

```text
str node
├─ len slot            // UTF-8 byte length, excluding trailing NUL
└─ payload bytes + NUL // wire payload includes one trailing 0 byte
```

### Branch Node

```text
branch node
├─ len slot
└─ child table
   ├─ [child_rel_off | child_type]
   ├─ [child_rel_off | child_type]
   └─ ...
```

Branch child rules:

- child offsets are relative to the enclosing branch base
- `child_rel_off == 0` means physical absence while preserving child type

That relative-offset design is what enables:

- subtree relocation
- direct indexed access
- lazy traversal

---

## Error Model

### Encode

`TreeEncoder.encode()` uses throwing semantics.

It throws on:

- malformed graft packet
- packet size overflow
- structurally invalid input reaching the encoder

### Read

`TreeReader` uses throwing semantics.

It throws on:

- malformed packet header
- malformed payload bounds
- malformed child offsets
- OOB traversal
- malformed leaf payloads

### Generic Write

Generic write methods use throwing semantics.

- success -> writes into the current tree
- unsupported generic input -> throws
- generic `bigint` outside `i64` range -> throws

This keeps the generic lowering path strict, while explicit writers remain caller-directed constructors.

---

## Design Summary

This tree packet format aims for:

- easy default DX
- explicit control when needed
- compact binary layout
- shallow and deep read modes
- lazy traversal
- typed-array friendly transport
- preserved typed optional metadata
- no hidden schema semantics

Use it when you want:

- structured binary packets
- packed typed-array transport
- lazy tree traversal
- a lower-level substrate for higher-level protocols
