# sys

The core cannot read a file, open a connection or tell the time. What serves
it across xpute's link does all of that for it. What the core has is its
memory, laid out once, and the entries the link gives it. `sys/` is the layer
that makes those enough. It turns what the layers above want into calls across
the link, fits every answer
into memory that was set aside at boot, and spends each entry's time in an order
the rest of the core can rely on.

`sys/` imports nothing from the domain, the renderer or the profiler, and
`cargo check-rules` refuses such an import.

## The memory, laid out once

The core's memory is the manifest's `mem.sz`, whole before its first entry and never
grown, so a full heap stops the core. Its sections never move.

```text
 0 ┌──────────────┐
   │ stack        │ first, so an overflow traps below address 0
   ├──────────────┤
   │ data         │ the linker's
   ├──────────────┤
   │ .fixed       │ ranges sized at compile time, never freed: the terrain,
   │              │ the divisions, the body slots, the uploads
   ├──────────────┤
   │ heap         │ everything else, under one allocator
   └──────────────┘
```

A range belongs in `.fixed` (`range.rs`) only when its owner can say at
compile time how large it will ever be and it is mostly in use whenever the
core runs. That is true of a few of the core's own formats and of nothing else:
a glTF model or a PNG has no bound, so the loader reads it into the heap.
`link/layout.rs` lays the ranges out and hands each owner its place at boot;
the manifest declares `.fixed`'s size and the heap the core counts on past it,
and the compiler and the boot check that the memory holds both.

The heap (`heap.rs`) is the global allocator: a slab of size classes over a
buddy allocator of pages, whose blocks are aligned to their own size, which
bounds fragmentation. What is held only at times lives on it, the largest of it
under caps that the build checks against the heap.

The largest bodies, terrain and Earth packets of about two mebibytes, take
slots in `.fixed` instead (`io/body.rs`): a fixed number of them, each as long
as the longest such body. A slot is held by its guard from the moment it is
taken until the guard drops: a read lends it and its answer hands it back, and
a layer holds it until it has decoded the body, or an Earth body until a drawn
frame takes its texels. With every slot out a body waits for one given back.
Every other body is a heap buffer of its own, owned by whoever holds it.

## An entry

The core runs only when xpute's link enters it: with a call from the UI, an
answer, or the answer of a wait it armed. What it does there is its own, on
embassy's executor (`sched.rs`). A frame is asked for (`invalidate`, or
`invalidate_at` for a time, one clock wait armed for the earliest) and runs at
the display's next vsync as one task, in one order: the effects of what the UI
sent, then the tick, then the draw (`frame.rs`).

Each entry has a budget from its start (`begin`, `param.rs` `BUDGET_MS`). The
tick runs the phases the core declares, in their order, within it
(`sched/tick.rs`), and a list of work stops once the budget is spent and one
item has done work, so the list always moves (`run`). A task yields where the
budget is spent and comes back in a later entry (`yield_if_spent`). An entry
ends once no task is ready, and a core with nothing to do is not entered.

## Reads

Every read the core makes goes through one scheduler, `io/request.rs`. A layer
does not start a read. It says, every frame, everything it wants on its stream,
each with its place in the view's order. The scheduler keeps what is still
wanted, drops what is not, and once every layer has spoken it runs one pump: it
stops what nobody wants any more, ranks the rest by the one policy
(`io/policy.rs`), and starts the best of them on the frame's credits.

```text
 a layer ── declare(stream, wants) ─┐
 a layer ── declare(stream, wants) ─┼─▶ request: one table of wants,
 the loader ─ declare(asset, wants) ┘    one pump a frame
                                              │ start / stop
                                              ▼
                               dispatch ─▶ fetch (tiles, objects)
                                        └▶ loader (assets)
                                              │
                                              ▼
                                  file ─▶ xpute's fs, the client's files
                                              │ completion
                                              ▼
                                   packet ─▶ the layer that asked
```

Because the wants are said whole every frame, the scheduler always works from
what the view needs now. A zoom passed through holds no credit: its reads are
stopped in the frame the camera leaves it. A read that is out is never
preempted for a better one; a better one waits for a free credit.

The policy is the only place where one layer's wants are ranked against
another's. It orders by how a view builds up: the ground first, then the
pictures drawn whole, then models, then places, each nearer and seen before
farther and unseen.

## When a read does not come back

A read can fail in three ways, and each holds back something different. When
the object itself is wrong, asking again brings the same bytes, so its want is
held for as long as it is wanted. When the heap or the slots refused the body
room, the want waits until what refused it gives room back, and only that
(`io/room.rs`). When the network or the server failed, every start is held:
one probe goes out at a time, after a wait drawn at random below its cap, so
clients that failed together do not return together. Each failure of the server
raises the cap, and after the last wait the server is given up, and the UI says
so.

None of this makes anything up. While a read is held, the view draws the
coarser tile it has, or nothing.

## Files the core keeps and calls it makes

The core keeps one file of its own, its settings, read and written whole. A
file of another format, or one it cannot read, is refused, never written over,
and set aside by the core, its backend keeping it elsewhere (`io/file.rs`).

Text is the one thing the core asks of its platform and waits for. A layout or
a raster is a call that the platform answers before it returns, writing the
answer into the buffer the call lends (`io/text.rs`).
