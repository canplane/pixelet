# The read direction: a change log, and everything else as an adapter over it

**2026-09-13** — BACKLOG P0-1 at the time. Once the truth is in a core, the UI has to
learn that it changed without the core knowing what the UI cares about;
without a protocol for that, the default is copying the world into JS
every frame
([20260912-why-the-destination-is-wasm.md](../20260912-why-the-destination-is-wasm.md),
"What it still costs"). The write direction has its shape — a numeric
command with an errno reply
([20260913-boundaries-commands-and-what-comes-back-from-april.md](./20260913-boundaries-commands-and-what-comes-back-from-april.md)).
This is the read direction's. Built the same day; see the last section.

## Constraints, set before the design

- **Not tied to an implementation.** Push, pull or signals; TypeScript or
  Rust; the main thread, a worker or the network — each is a way of
  delivering, built on the protocol, never part of it.
- **An interface.** What a source promises and what a reader may ask, stated
  as operations over data, so either side can be replaced alone.
- **Data-oriented.** Changes are records in lanes. A reader's interest is a
  loop over a span, not a callback per consumer per change.

## Where to look, and what was there

- **`checkpoint/20260405-xpute`** has no read direction. Searching the branch
  for subscribe, observer, on_change, dirty, revision and notify finds only
  the network transport's "WS is the notify/control plane". April had
  request and reply (commands, signals); its UI read zustand stores directly.
  What came back from it is the command's shape, and that is all it offers
  here.
- **Figma** does not say. Its multiplayer writing describes the document as
  `Map<ObjectID, Map<Property, Value>>` with property-level changes — the unit
  of a change — but not how its UI learns of one; the bindings layer is not
  described in anything published
  ([How Figma's multiplayer technology works](https://www.figma.com/blog/how-figmas-multiplayer-technology-works/)).
- **ECS change detection** is the documented precedent. Bevy keeps, per
  component, the tick it was added and the tick it was last mutably
  accessed; a system compares them with the tick it last ran, which is what
  `Changed<T>` and `Added<T>` filter on, and a periodic check clamps ticks so
  they cannot overflow
  ([bevy::ecs::change_detection](https://docs.rs/bevy/latest/bevy/ecs/change_detection/index.html)).
- **Immediate-mode UI** (egui, Dear ImGui) keeps no subscriptions at all: the
  UI reads state every frame.
- **This repository already pulls.** `chunk_node.ts` reads
  `object_column_revision(key)` each tick and rebuilds only when it moved,
  and those revisions are lanes in the core memory (`object/store.ts`).

## What reads today: six mechanisms

| # | Mechanism                                              | Where                                          | Delivery                                   |
| - | ------------------------------------------------------ | ---------------------------------------------- | ------------------------------------------ |
| 1 | a zustand store's `revision` and `useXStore.subscribe` | terrain, POI                                   | push, whole table                          |
| 2 | `register_tile_interest(matches, notify)`              | search, companion, puck, POI layer, chunk_node | a predicate callback per consumer per page |
| 3 | `ListenerTable.subscribe(key, fn)`                     | object store, by column and by cell            | push, per key                              |
| 4 | `PageTable.subscribe`                                  | page tables                                    | push, whole table                          |
| 5 | `useSyncExternalStore`                                 | an object cell, decor assets                   | React adapter                              |
| 6 | `object_column_revision(key)` read each tick           | chunk_node                                     | pull                                       |

Settings, the hotbar, the camera, the map view and the device's sensors are
personal or local state and stay outside the protocol — the split the
command channel already draws
([20260908-command-channel-for-state-mutation.md](./20260908-command-channel-for-state-mutation.md)).

## The protocol

**Three numbers.**

- `topic` u16 — a kind of state: a terrain page, a POI page, an object
  column, a decor column, an object cell, an asset bundle. Numbered, the way
  a command is.
- `key` u64 — the unit within a topic. A page index, a column base and a cell
  corner are all 64-bit already.
- `revision` u32 — the topic's tick at the key's last change. One counter per
  topic, not per key: a key forgotten and changed again gets a revision it
  never had before, so a reader holding an old one can never mistake the new
  state for the one it saw. This is Bevy's tick, per topic.

**A source does one thing:** `bump(topic, key)`. The topic's tick advances,
and the change is written twice:

- the **revision table** — (topic, key) → revision, as lanes; `rev_map` and
  `rev_objects` in the object store are already this;
- the **journal** — one fixed-capacity ring for every topic, each entry
  `{ topic u16, key u64, revision u32 }` as lanes.

A source may also `forget(topic, key)` when a key leaves (a page unloaded),
which drops its row from the table; the journal is unaffected.

**A reader may ask two things, of the same data.**

- **Point read** — `revision(topic, key)`: different from the one remembered
  means changed; 0 means never seen, or forgotten. What mechanism 6 does.
- **Journal read** — the entries from its own cursor to the head, as a span
  over the lanes. Interest is the reader's loop over that span: a page filter
  is a comparison in a loop, not a closure the source calls. Replaces 2.

**Overflow.** A cursor the ring has overwritten reads as `lost`. The reader
then re-reads the whole of the topics it cares about and takes the head as
its cursor. There are no per-reader queues, so memory is fixed whatever the
number of readers; Bevy clamps ticks to the same end.

**No values in a change.** A change says which unit moved and to which
revision. The value is read from its owner's lanes by key, in place. So the
journal is small and fixed-width, and a span of it crosses a boundary as a
frame body unchanged.

**Every current mechanism is an adapter.**

- 1 and 4 — a topic's tick.
- 2 — a journal read with a page filter.
- 3 and 6 — point reads.
- 5 — once a tick, read the journal and wake the keys a component subscribed
  to; the component's `useSyncExternalStore` snapshot is the revision.
- A worker's or a server's mirror — the journal span, sent as a frame.

## Decided with the design

- **One journal**, with the topic as a column, not one per topic: a reader
  holds one cursor.
- **`lost` means re-read the topic.** No partial recovery.
- **Local state stays out** (settings, UI, camera, map view, sensors).

## Where it goes

`@xpute/core/ipc/change.ts`. Nothing in it is pixelet's: topics are numbers a
caller assigns. The command is the write half of the channel and this is the
read half.

## Open

- **Keys are BigInt in TypeScript.** The table is keyed by the IndexedMap the
  repository already uses for 64-bit keys, and reading a key back from its
  lane makes a BigInt. That is the cost the Rust core removes; the layout —
  a u64 — is the part that has to be right now.
- **A topic tick is u32.** Four billion changes to one topic before it wraps;
  a wrap would have to clamp readers the way Bevy's check does. Not handled
  in the first version, and said so in the code.
- **The order the six move in.** 6 and 3 are closest to the shape already;
  2 is the one that removes the most callbacks. Decided when building.

## Built, 2026-09-13

- **The log** — `@xpute/core/ipc/change.ts` (c2b1364), as above.
- **Push delivery** — `@xpute/core/ipc/change_waker.ts`: listeners per
  (topic, key), and topic listeners that hear each changed key, once per key
  per pump, null when the pump lost its place.
- **The world's log** — `module/world/change.ts`: six topics (object
  column, decor column, object cell, terrain page, POI page, decor asset),
  over the core pool; `bump_world` records a change and asks for a frame,
  because the canvas renders on demand; the tick pumps before its first
  phase.
- **The six mechanisms**, each now an adapter:
  - 3 and 6 — the object store's revisions are `revision_of`, its column and
    cell listeners are the waker's (5a573af).
  - 1 and 4 — a PageTable given a topic bumps it on every set and clear and
    forgets a page that left; its revision is the topic's tick, which the
    zustand stores mirror for React (a9f60aa).
  - 2 — `register_tile_interest` keeps its signature as a filter over the
    terrain topic (a9f60aa).
  - 5 — decor asset arrival is its own topic; `subscribe_decor_assets` calls
    once per pump in which anything arrived, and `useDecorAssets` stays a
    `useSyncExternalStore` over it.

**What moved in behavior:** every listener runs at the start of the next
frame, never inside the write that caused it. Checked by hand in the running
app after the object and page steps: terrain fills, search re-grounds, the
puck and the POI badges sit on the ground. The decor-asset step is checked
by the tests only so far.

**Not in it:** settings, the toasts and the fetch queue's status keep their
own subscriptions. They are local state or scheduling, the split this
document drew at the start.
