# One command channel for state mutation — local storage and a future server are just different backends for it

**2026-09-08** — prompted by a conversation, not a code change: login/session
and multiplayer are coming up next, and today's session happened to add a
second zustand `persist` store (`ui_store.ts`, mirroring `settings_store.ts`)
right before that discussion. The question this records: should "save to
localStorage" and "sync through a server" stay two separate mechanisms that
get bolted together later, or should they be the same interface from the
start, pointed at different backends?

## Two kinds of state that currently look the same

Everything mutated through a zustand setter today goes straight to
`localStorage` (via `persist`) or nowhere (in-memory only). That flat
treatment hides a real split:

| Kind                             | Examples today                                                                                                                                               | Does another player need to see it?  |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------ |
| Personal / local-only preference | `ui_store.ts`'s `recent_picks`/`object_color_id`/`decor_id` (hotbar), `settings_store.ts` (graphics/camera/companion picks)                                  | No — never, even with a server       |
| World-shared state               | `objects/store.ts`'s placed cubes/decor (`place()`, already its own `localStorage` key, `pxl:object:v5`), and eventually chat, POI edits, character position | Yes — this is what multiplayer means |

The first kind should stay exactly what it is forever, server or no server —
nobody else needs to know which color you last painted with. The second kind
is the one that actually needs a server to become the source of truth once
other players exist.

## The idea: one dispatch interface, swappable backend

Rather than two parallel systems (a `localStorage` write path for solo play,
a server round-trip added on top later), route every world-shared mutation
through one command-dispatch call from the start — `place_cube(...)`,
`erase(...)`, `send_chat(...)`, whatever the eventual set turns out to be —
and let _that one function_ decide where the command actually goes:

- No session / no server reachable: apply it to local state directly (today's
  behavior, unchanged).
- Logged in, connected: apply it optimistically to local state **and** ship
  it over the wire (websocket, almost certainly — bidirectional, low-latency,
  fits a steady stream of small typed commands better than repeated HTTP
  requests), with the server as the arbiter once it responds.

Calling code (a component, a store's own action) never branches on "am I
online" — it just dispatches. The backend swap happens in one place, below
every call site, not scattered across every feature that mutates world
state.

**Corollary worth stating explicitly, since it wasn't obvious until said out
loud:** if this is built right, single-player and multiplayer stop being two
different _structures_ — single-player is just "the dispatch backend is
local-only, zero peers," not a separate code path that multiplayer support
gets bolted onto later. That symmetry is the actual payoff of doing this now
rather than after a server exists: it removes an entire axis of divergence
(solo-mode logic vs. multiplayer-mode logic) before it has a chance to grow.

## Why this isn't a rehash of the old P2P/BFT direction

[20260708-vision-and-scope-pivot.md](./20260708-vision-and-scope-pivot.md)
parked a much more ambitious version of "every mutation is a networked
command" — peer-to-peer, Byzantine-fault-tolerant, no trusted server at all —
and that collapsed for reasons unrelated to this idea (NAT traversal doesn't
close on the web, BFT consensus cost). This is a conventional
client-to-server model: a real server is the trusted arbiter the moment one
exists. What's being proposed here is narrower and purely about _where the
branch point lives_ — one interface with a swappable backend, instead of
local-only code that later grows an if-online-do-this-instead branch
throughout the codebase. It doesn't need trust, consensus, or P2P to be
worth doing.

## What already fits this shape, and what doesn't yet

`objects/store.ts`'s `place()`/erase path is already close: a small,
well-typed mutation (coordinate + size + color/decor id) against a sparse
tree, already serializable, already closer to "apply this operation" than to
"here's my whole new state." Turning that into a real command is mostly
giving it an envelope and a transport, not redesigning the operation shape.

`ui_store.ts`/`settings_store.ts` are correctly _outside_ this system — see
the table above — and should keep talking to `localStorage` directly through
`persist`, same as today.

## Open, not decided here

- Transport is assumed to be a websocket; not committed, since there's no
  server yet to design the protocol against.
- Command schema/versioning (a real question once this exists — object
  storage already has its own `OBJECT_STORAGE_VERSION` precedent to learn
  from) and whether `xpute`'s XTP format (mentioned as a possible spin-out in
  the vision-pivot doc) is a fit for the wire encoding.
- Whether existing setters (`place()`, `select_cube`, ...) get wrapped or
  rewritten when this is actually built — this document is the reasoning to
  have on hand when that happens, not the design of the dispatch layer
  itself.
- What "session" (the login work this was prompted by) needs to exist before
  any of this is reachable at all.

## Status

Discussion only — no code changed. Revisit when server/login/multiplayer
work actually starts; nothing here should block or shape work before that.
