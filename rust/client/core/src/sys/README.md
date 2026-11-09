# sys

The core cannot read a file, open a connection or tell the time. The runtime
owns all of that, as a virtual machine monitor owns the devices of its guest.
What the core has is its memory, laid out once, and the turns the runtime gives
it. `sys/` is the layer that makes those enough. It turns what the layers above
want into asks of the runtime, fits every answer into memory that was set aside
at boot, and spends each turn's time in an order the rest of the core can rely
on.

`sys/` imports nothing from the domain, the renderer or the profiler, and
`cargo check-rules` refuses such an import.

## The memory, laid out once

The core's memory is one linear space whose sections never move after boot.

```text
 0 ┌──────────────┐
   │ front        │ the stack, the header, the rings, the GPU stream,
   │              │ the linker's data: laid out by xpute's machine
   ├──────────────┤
   │ .fixed       │ ranges placed once at boot, sized at compile time,
   │              │ never freed: the terrain, the divisions, the uploads
   ├──────────────┤
   │ heap         │ everything else, under one allocator
   └──────────────┘
```

A range belongs in `.fixed` only when its owner can say at compile time how
large it will ever be and it is mostly in use whenever the core runs. That is
true of a few of the core's own formats and of nothing else: a glTF model or a
PNG has no bound, so the loader reads it into the heap. What is held only at
times lives on the heap as well, the largest of it under caps that the build
checks against the heap.
`door/layout.rs` decides which ranges `.fixed` holds and how large each is, and
hands each owner its range at boot. The machine names how much heap the core
must keep, and both the compiler and the boot check that the front and `.fixed`
leave it.

The largest bodies, terrain and Earth packets just past a mebibyte, would cost
the buddy allocator two mebibytes each. They take slots instead: fixed-size
pieces cut from runs the heap gives whole and takes back whole, a slab
allocator for one large object (`io/body.rs`). The number of slots out at once
is bounded, so those bodies take no more than their runs however they arrive.

## A turn

The runtime decides when the core runs. Each turn arrives with a grant that
says how long the frame may take and how many reads may be out, and the core
spends it in one order: the effects of what the UI sent, then the tick, then the
draw, then the tasks (`turn.rs`).

The tick runs the phases the core declares, in their order, against one
budget. A phase that runs long cannot take everything: each phase holds back a
share of what is left for the phases after it, so a busy load does not starve
what follows (`sched/tick.rs`). After the frame, the turn's tasks get their
own share of the frame time, however far the frame ran over (`sched/edge.rs`).
A task that has work left asks for the next turn; a core with nothing to do asks
for none, and the runtime lets it sleep.

## Reads

Every read the core makes goes through one scheduler, `io/request.rs`. A layer
does not start a read. It says, every turn, everything it wants on its stream,
each with its place in the view's order. The scheduler keeps what is still
wanted, drops what is not, and once every layer has spoken it runs one pump: it
stops what nobody wants any more, ranks the rest by the one policy
(`io/policy.rs`), and starts the best of them on the credits the turn granted.

```text
 a layer ── declare(stream, wants) ─┐
 a layer ── declare(stream, wants) ─┼─▶ request: one table of wants,
 the loader ─ declare(asset, wants) ┘    one pump a turn
                                              │ start / stop
                                              ▼
                               dispatch ─▶ fetch (tiles, objects)
                                        └▶ loader (assets)
                                              │
                                              ▼
                                  file ─▶ the runtime's file device
                                              │ completion
                                              ▼
                               body block ─▶ the layer that asked
```

Because the wants are said whole every turn, the scheduler always works from
what the view needs now. A zoom passed through holds no credit: its reads are
stopped in the turn the camera leaves it. A read that is out is never
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
and named to the client, which sets it aside (`io/file.rs`).

Text is the one thing the core asks of its platform and waits for. A layout or
a raster is a call that the platform answers before it returns, writing the
answer where the call said (`io/text.rs`).
