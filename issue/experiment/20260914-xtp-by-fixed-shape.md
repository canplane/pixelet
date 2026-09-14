# XTP by fixed shape: what a ring query pays

**2026-09-14.** A ray query through the rings (`RAY_READ`) cost 733 ns
where the export it replaced cost 91 ns, and a ring call with no packet
cost 92 ns — so the difference was the packet, not the ring
([what stays in JavaScript](../20260914-what-stays-in-javascript.md)).
The question: is that the price of a format without a schema, or of the
TypeScript encoder and reader as written? Measured once, not built.

## Method

V8 (Deno), the core module over an empty world, 300,000 calls after
20,000 of warm-up, on the same machine as the earlier numbers. Four
ways of making the same query:

- the request built through `TreeView` and `encoder.encode`, as now, or
  written by fixed shape — the packet for "a branch of one nine-double
  array" encoded once, and only its nine doubles overwritten per call;
- the reply read through `TreeReader` and a cursor, as now, or by offset —
  the fourteen doubles are the packet's tail, read as an index into a
  `Float64Array` over the memory.

A miss carries no reply packet, so the reply side was also timed on its
own over a hit-shaped packet in the module's memory.

## Numbers

| Query                                     | ns a call |
| ----------------------------------------- | --------- |
| TreeView request, TreeReader reply (now)  | 709       |
| TreeView request, reply read by offset    | 693       |
| fixed-shape request, TreeReader reply     | 216       |
| fixed-shape request, reply read by offset | 215       |

| Part                               | ns a call |
| ---------------------------------- | --------- |
| encode the request: TreeView       | 492       |
| encode the request: fixed shape    | 54        |
| read a 14-double reply: TreeReader | 303       |
| read a 14-double reply: by offset  | 5         |

## Reading

- **The cost is the TypeScript writer and reader, not the format.** The
  same bytes, written by shape, cost a tenth; read by offset, a sixtieth.
  XTP is binary and read in place already.
- **A query becomes ~215 ns** with the request written by shape; a hit
  adds ~300 ns through `TreeReader` or ~5 ns by offset. The rest over the
  export's 91 ns is the push's copy into the arena, the drain and the scan
  of the completion ring.
- **A shape known ahead is what makes it cheap.** Writing by shape is what
  code generated from the command table would do: the header, the branch
  table and the array header fixed, the leaves written at known offsets,
  and the reply's leaves read at known offsets — with the types checked at
  compile time as a side effect. That is the schema layered on XTP, not a
  second format beside it: FlatBuffers would buy the same with a builder
  that is not free in JavaScript, and would split the wire format the
  pages, records and rings already share.

## What it leaves

BACKLOG P1: a writer and reader generated from `@pxl/shared/spec/cmd.ts`
per command, over XTP, for both sides. The experiment code was not kept.
