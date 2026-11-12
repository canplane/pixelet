# xpute as objects, references and methods

**2026-11-12** — recorded with the user. Main was at `4d95242546` when this was
written, with step 2 of the pixelet rewiring uncommitted. It replaces the
morning's minute of the same name. It gathers the axes the user holds the rework
to, what was decided along them, and the model xpute now has. Who decided is said
where it was the user, or reduce-s23-light-p95 within what the user gave it; the
rest was proposed by the agents and taken by them.

## What xpute is

xpute binds the native and the web platforms into one: a program's core runs the
same on a page and on a device. It is a foreign function interface between the
core and its runtime, and nothing more. It has no screen, no frame and no
scheduler. Those are the program's. It is to be published, and is judged as if
pixelet were only its first user.

It holds, for one boundary that spans wasm and native, the place the Canonical
ABI holds in the component model. Its layers correspond:

| Layer | Component model | xpute |
| --- | --- | --- |
| Interface | WIT | WIT |
| Bindings | wit-bindgen | xpute's generator |
| Boundary ABI | Canonical ABI (lift, lower) | the frame, XTP and the link |
| A resource's reference | an integer handle, `ResourceTable` | an integer handle, the handle table |
| The host's implementation | registered in Wasmtime's `Linker` | the opener and handle tables, polymorphic behind them |

The differences are chosen. One passage carries every call where the component
model links each function, since a device's FFI, the UI's port and completion by
TAG all assume one. Memory is lent in place for the call or until the
completion, where the Canonical ABI copies and ends a borrow at the call's
return. Only the vocabulary of own and borrow is taken from it. XTP stays the one
encoding.

## How the work is judged

The user holds every change to these:

- **Structural reduction first, and the least state.** Each change says what state
  it removed, or what an added state buys.
- **Bring in only what is truly needed**, and only after the structure it joins is
  reduced. That covers old code from history, crates, flags and the machinery of a
  convention. When it is unclear, it stays out.
- **No tests or conformance at this stage.** A plausible skeleton, close to
  pseudocode, comes first. A hook blocks running tests.
- **No ad hoc pieces.** Cut them on sight, in the same step.
- **Convention, checked against its source.** Each source is verified before it
  is relied on.
- **Who decides**: the user decides. reduce-s23-light-p95 decides within that and
  reviews before work reaches main.

## What happened today

- The xpute-ring line was taken off main: main went back to `origin/main`, and the
  24 commits are kept on `keep/a7347-ring-24`. The regression that had a program
  implement xpute's own protocols (`a7347a3152`) went with it. The first-launch
  fix was carried back.
- The user then deleted xpute's core and both runtimes, breaking the build on
  purpose, so the boundary would be written again from its interface and not from
  the old structure.
- The rings were judged.
  - **Late September: excessive, partly an operating system's analogy.** They were
    introduced to save a crossing that cost nothing: 92 ns through a ring against
    91 ns through the export it replaced, and 1,752 ns against 202 ns for a pool
    block. No thread boundary existed for a ring to span, and the slot sizes came
    from io_uring and NVMe, not from traffic.
  - **Their removal on 2026-11-11: justified.**
  - **Today's direct link: sound for a core on its own thread.**

## The link

- The core imports `link_out(at)` and exports `link_init()` and `link_in()`. These
  three are the transport. They live in the link and the generated bindings, not
  in any WIT world.
- A frame carries four words. TAG is the request's id, from the sender's counter.
  The counter wraps, skipping tags still pending. CMD names the target and the
  method, with one RES bit. RESULT is a value ≥ 0 or a negative errno. PKT is the
  XTP packet's length. XTP stays the args' encoding.
- A pointer is an offset into the core's memory and a length, `(at, len)`. Offset
  0 is not special. A pointer lent to a call is valid for the call; one lent to an
  async request is valid until its completion. Data crossing to another thread,
  as to the UI, is copied: it travels as bytes in the packet.
- The core uses its module's own growable memory. Every read goes through the
  memory's getter, never a cached view.

## Calls and async

- There are three ways on each side: `call` answers at once, `send` takes an
  optional answer, and `on` registers a handler.
- There is one async mechanism. A handler that cannot answer now returns
  EINPROGRESS and later completes the request with a RES frame of the same TAG. An
  `async func` always answers later, so awaiting one leaves the core's entry.
  `call` is only for functions that answer at once, and EINPROGRESS to a call stops
  as a bug.
- An answer is never delivered inside `send`, in line with Promises/A+. On the
  core, answers queue in DUE and run at the end of the entry, until none is left,
  as microtasks are drained. On the runtime, they run after the current work
  (`queueMicrotask`).
- There is one core entry at a time. ENTERED checks it, since a runtime handler
  can re-enter the core synchronously.
- RES carries a return packet. A `send` answer copies its packet and owns it. A
  `call` answer's packet is readable until the next crossing.
- xpute gives thin adapters and no executor. Rust's `wait` is a lazy future: the
  send happens on first poll, and dropping the future abandons the result without
  cancelling it. A lent buffer is held by the pending answer until the completion.
  TS has a `wait` that returns a Promise.
- There is no backpressure for now. A serving resource that needs it answers
  EAGAIN or EBUSY under its own contract.

## Time and frames

- The turn, the wake and the quota are gone from the boundary. The core runs only
  when it is called: a request, a completion, or the completion of what it armed.
- `timer` and `clock` are xpute's general host services, as `wasi:clocks` is.
  `vsync` belongs to xpute-gpu, since a display's refresh is the surface holder's.
  A program imports only what it uses.
- The app's lifecycle is xpute's: `xpute:core/life`, with `hidden` and `shown`.
  xpute defines it, a program that cares exports it, and xpute's runtime calls it
  from `visibilitychange` on a page and from the app's lifecycle on a device.
  `hidden` is the core's last sure moment to save. It is a call like any other,
  taken at the core's next entry and never interrupting one.
- Inside pixelet, embassy stays the executor, and only the turn's glue went.
  - At each entry's end, the link drains DUE and pixelet polls embassy until both
    are quiet. The UI post then follows, so a woken task never waits for an entry
    that may not come.
  - "Want a frame" is one signal, `invalidate`. Timed wakes keep one earliest time
    armed. A per-entry deadline is read from the clock.
  - A yield is `sleep(0)`. The browser's 4 ms clamp on nested timeouts is open.
- Auto resolution and the auto frame rate both stay.
  - The rate is paced in the core by skipping vsyncs to a whole number of measured
    display intervals.
  - It is judged by the observed frame interval and the GPU time already recorded.
  - Resolution's effect is measured: half the pixels bought 12% of the S23's GPU
    p50. Pacing's own effect was never measured apart from resolution. Fewer
    intervals were missed (45% against 87%) only with both changed at once, so it
    is to be measured on a device.

## Interfaces

- The interfaces are written in valid WIT, as the description only. The execution
  meaning stays xpute's. A generator reads them with wit-parser, limited to the
  subset in use: interface, func, async func, resource, record, enum, variant as
  far as a token needs, option, list, integers and string. It refuses the rest and
  writes the Rust and TypeScript bindings.
- A singleton service is an interface's functions, not a resource. The core's
  commands are split by what they were prefixed with (camera, input, scene,
  sensor, setting, edit, source, perf). A program calls `camera.goto()`.
- The parties keep their names: core, ui, runtime. The interface the UI implements
  is `ui`; no other name is introduced for it.
- The snapshot's records become WIT records. A format with its own meaning, such
  as the GPU stream, stays as it is.

## Naming what is called

Command, file and handle are one model: an object is named, opened to a reference,
and its methods are called. This keeps the minute's earlier table, and adds how a
name becomes a number.

- Everything that can be called is named by a URI: a tuple of tokens, each an i32
  or a string. A singleton interface is named by its package and interface, and a
  file by the program's path tokens.
- The root, handle 0, has the one fixed method: `open(uri)` returns a handle. That
  is the bootstrap every comparable system has: Binder's handle 0, COM's IUnknown
  and Fuchsia's namespace. It merges 9P's walk and open into one step.
- CMD holds the handle and the method's number within its interface, in
  declaration order. Both ends are built together, and an interface is reached by
  name, so a package reordering its interfaces moves nothing. Singleton interfaces
  are opened once at boot. A file is opened when needed and closed when done.
- Behind the handle, the serving side is polymorphic. An opener table picks the
  implementation by the URI's leading tokens, as mounts do, and the handle table
  holds the object. Each implementation keeps its own lifetime rules. Only the
  serving side is polymorphic, so the caller never branches on what it calls.
  pixelet's queries between its render and storage layers follow the same idea.
- Rejected along the way:
  - numbers from declaration order across a world, which change with what a world
    includes;
  - a hash of each function's name, FIDL's way;
  - a hash of the package with ordinals inside;
  - one global sort of every function, which breaks xpute's prebuilt services.

  RFC 9923's FNV-1a was checked as the hash before naming by URI made it
  unnecessary.

## Open

- **Native runtime:** delivery across threads (a queue and a lock), the core
  memory's base, and the link's C symbols.
- **Still to wire:** the file resource on the link, xpute-gpu's control functions
  (its runtime still names the deleted xpute-runtime), and the boot installs (the
  stop handler, `boot::set`, `layout::install`).
- **pixelet:** it compiles again and runs on the web and devices. Resolution and
  pacing are then measured apart.
- **Small items:** `BUDGET_MS`'s value, regenerating the perf spec, and EDEADLK in
  the errno spec.
- **C++ catches up last.**
