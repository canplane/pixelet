# xpute takes WIT's and WASI's words, and the native runtime's shape

**2026-11-13** — recorded with the user. Main was at `021afd1dbc` when this was
written. It follows `20261112-xpute-as-objects-references-and-methods.md`. Every
item says who decided it: **the user**, or **the agents** (reduce-s23-light-p95
and another session) within what the user gave them. An agent's plan is never written
as the user's decision.

## The rule the user set

xpute aligns its words and its structure to WIT, WASI and the component model,
and implements its own only where that is truly needed (**the user**). It
depends on none of them at run time: the core is a plain wasm module or a static
library, the link carries every call, and `wit-parser` is used only by the
generator. It borrows the IDL, the names and the shapes.

In the standard's terms the core is the guest and the runtime the host. The UI is
outside WASI: it calls the guest's exports and serves some of its imports, and
only xpute's WIT draws that boundary.

## What was aligned

- **Clocks.** `wasi:clocks@0.3.2`, as a subset (**the agents**, on the rule):
  `types` with `duration`, `monotonic-clock` with `mark`, `now` and `wait-for`,
  and `system-clock` with `instant` and `now`. The interface `timer` and the old
  `clock`, which wrote into the core's memory past `lent`, are gone. Nothing
  else of the package came in.
- **Drop and task return.** The method that lets a handle go is `drop`, the
  component model's resource drop, and `close` stays apart because it reports a
  commit. A later answer is `task_return`, the component model's `task.return`
  (**the agents**).
- **Handles.** Slots are reused first in, first out, as the component model's
  table reuses them, with a generation in the handle against a late frame and an
  issued mark so that a drop of a free slot is refused (**the agents**).
- **A packet past the room** is refused as EMSGSIZE on both sides, and an answer
  that does not fit reaches its requester as EMSGSIZE, whether given at once or
  later (**the agents**).

What stays xpute's own, and why (**the agents**, on the rule):

- **The link**, one passage for every call. A browser runs no components, and
  the same core is served from a worker, across a port to the UI and natively.
- **Lending in place** by `(at, len)`. The canonical ABI copies; the core and
  its runtime share one memory, and a tile body cannot come back as return data.
- **Naming by URI** and the mount table. Mounts are made at run time, by prefix.
- **fs**, with `changed` and a `close` that reports its commit, which WASI's
  filesystem has no form for.
- **stop**, which carries a structured stop code, and **life**, which WASI 0.2
  has no interface for.

## The GPU is a plugin

The dependencies run gpu → core only (**the user**). xpute-core, its kit, its
runtimes, its WIT and its manifest schema name no GPU. What the program asks of
the GPU, its power and features and the stream's size, is the program's choice
and lives in pixelet's render spec, which the page passes to the plugin. The
generator carries no plugin's code.

## Data specs are translated one to one

Generation is a one-to-one translation of the JSON, with nothing interpreted in
between (**the user**). A string stays a string: a colour is `#rrggbb` in the
JSON and in every generated file (**the user**), and the one reader, pxl-spec's
`rgb`, is a `const fn` checked at build time, as other spec facts are
(**the agents**, on the user's word that it be handled as the other code is).
Generated code keeps only what the compiler uses, so a table's keys are not
written (**the user**). The u64 hex
strings, the count written for a list of names, and the generator's colour type
are gone. A `$type` the generator does not know is refused. Slot types, which
turn a path into a catalog number in the generator, are still an exception and
wait for the catalog's redesign.

## The page

- The worker's console reaches the page (**the user** approved it), and so do a
  trap and a start that failed, each once. A start that failed shows the boot as
  failed, not as stopped.
- The runtime's lines reach the measuring tool again: the worker's lines are
  written through the page's `log`, which a measuring build keeps in
  `globalThis.lines`.
- Touches go from the UI to the core on both platforms, through the same input
  driver, so the UI's rules for the world run on a device too (**the agents**, on
  the user's rule that two paths are no symmetry). The device's shortcut that posted touches straight to the core is
  gone. The line in the 2026-11-09 minute that had the runtime take touches
  directly was an agent's plan, not the user's decision, and was removed from it
  (**the user**).

## The native runtime, paused

Native work stops here; neither it nor C++ is widened now (**the user**). The C++
guest runs on the link as the Rust guest does: `xpute/cpp/xpute/link`
holds the link machine, `xpute/cpp/xpute/guest` the guest's end, and
xpute-link-gen writes its calls and serves. The old core, the conformance core
and their tests are gone, and the C++ builds natively and for wasm32. What was
decided before the pause:

- The runtime runs on the core's thread, as the page's worker holds the runtime
  and the core together (**the user**, on the agents' proposal). The core thread
  waits on one task queue, and other threads only push to it.
- One link machine, two instances: the core's and the runtime's (**the user**
  approved it).
- The GPU runs and presents on its own thread, since a present can wait on the
  frame before; `run` answers once its records are submitted (**the agents**).
- On native, every Rust allocation in the app lies in the one 512 MiB region,
  behind one lock (**the user**).
- Native builds unwind, so a panic in the core is a trap that stops the core and
  leaves the UI (**the agents**).
- The program builds its host as one list, its servers in no order with its
  world's `IMPORTS` and `LATER`, and each runtime runs the fixed steps itself, on
  a page and on a device alike: it mounts each server, serves the UI's side,
  enters the guest only once every import under a named handle is mounted, tells
  it the UI attached, and on a device runs the loop (**the user**). An import is
  checked by interface, an opened one included: `IMPORTS` lists every interface
  the world imports, and the runtime refuses to start where no server serves
  one, under its named handle or by an opener mounted for it at some prefix;
  which prefixes the guest opens stays the program's (**the agents**).
- What a path is bound to is the program's on both platforms; xpute offers only
  the platform's storage (**the agents**).
- A device has no disk of xpute's and no thread for one (**the user**): its
  storage is the platform's, as a page's is, through the client's
  `service/platform/storage.native.ts`.
- The runtime's composition is pxl-runtime (`rust/client/runtime`), the device's
  counterpart of the page's worker, with the device's server of pixelet's own
  text interface and its bindings; the device's library is built from it, and
  pxl-core is a guest that names no host (**the user**).
- The runtime's environment and its host split as the web's do (**the user**).
  The runtime is the environment: the event loop, wait, the log, JNI, the
  lifecycle, the link that enters the guest and the UI's channel. The host keeps
  the clock and the stop, with the link's host end and the host-side bindings.
  The host varies by language and the runtime by platform, and the runtime
  injects into the host, which names no runtime
  ([xpute's parts](./20261115-xpute-s-parts-cut-by-what-they-are-and-what-they-vary-on.md)).
- No new platform module (**the user**). A device's origin paths are served by
  the UI's JavaScript, through `service/platform/origin.native.ts` `get`, so the
  Rust has no HTTP client: the worker fetches them on a page and the UI's thread
  on a device.

## How the work is reviewed

Resident reviewers read each step adversarially, against AGENTS.md, the skills and
the memory, and the agents check each other's decisions the same way. A
collision between two names or two pieces is answered by reviewing the whole
context that holds them and which layer owns the fact, never by stepping around a
name already taken (**the user**).

## Open

- Renaming core to guest and runtime to host, and splitting the runtime's
  environment from its host along WASI's line of responsibility. The user asked
  for an opinion, and it is undecided.

  > Since 2026-11-13 (`73e2174df5`): the core is the guest, its server the
  > host and the platform's environment the runtime (**the user**).
- The catalog's redesign, which removes the slot types and the last exempt cycle.
- C++ catches up last.
