# Resources, files and streams

**2026-09-30** — Whether xpute's boundary could be abstracted the way Unix
abstracts its own — descriptors, files, pipes, system calls — or the way Plan 9
does, everything a file served over one protocol. Before any API, this sorts
every command pixelet sends or raises by what it actually is, and keeps apart
the ones that do not fit. No code changed.

## What is already Unix-shaped

A message is a system call: a number, arguments in a packet, and a result that
is a value or `-errno`. The two rings are io_uring's submission and completion
queues. A handle is a descriptor, with a generation so a number reused does not
name the old object. The stream, a turn's calls laid end to end, is a `writev`
of calls. And a command number is a subject and a verb, `major << 8 | minor`,
with four verbs every subject has — create, read, update, delete — and its own
from `MINOR_OWN`: a device and its `ioctl`.

What is not: the verbs are not common across subjects, there is no name to open
anything by, and data that flows — a pointer, a position — arrives as one
message after another rather than from something opened once.

## Every command, sorted

Each command is read on ten axes: whether it is a resource, whether it has a
lifetime, which way it goes (H→G host to guest, G→H guest to host), how often,
when it completes, what it carries, whether order matters, what names it, which
Unix verb it would be, and whether that verb fits without losing meaning or
adding state. `apps/web/src/kernel/abi/cmd.spec.ts` is the command list; the
host's stream is `spec/pxl/web/kernel/abi/host.json`. The device's four C calls
are not here: they are its host's own services, and the kernel learns of what
they did by the same commands as on the page
([20260930-sync-and-async-across-the-boundary.md](./20260930-sync-and-async-across-the-boundary.md)).

### Resources

| Command                                                              | Resource               | Lifetime | Way                           | How often              | Completes               | Carries               | Order   | Named by                                      | Unix                       | Fits                                                                      |
| -------------------------------------------------------------------- | ---------------------- | -------- | ----------------------------- | ---------------------- | ----------------------- | --------------------- | ------- | --------------------------------------------- | -------------------------- | ------------------------------------------------------------------------- |
| `SURFACE_CREATE` / `UPDATE` / `DELETE`                               | a canvas               | yes      | H→G                           | once / repeated / once | call / next turn / call | XTP record            | yes     | a handle the guest issues                     | open / ctl / close         | yes                                                                       |
| `PREVIEW_CREATE` / `DELETE`                                          | a surface's mode       | yes      | H→G                           | once                   | next turn               | XTP (`surface`)       | yes     | the surface's handle                          | ctl on its fd              | only as a ctl; opened on its own it would not                             |
| `FETCH_CREATE` / `DELETE` (raised), `FETCH_UPDATE`, `BODY_READ`      | a remote object's read | yes      | opened G→H, done H→G          | once                   | next turn               | XTP, then a byte span | per key | the guest's domain key and a host body number | open / done / read / close | the flow is Unix's; one descriptor for both names needs a key-to-fd table |
| `LOAD_CREATE` (raised), `LOAD_UPDATE` / `LOAD_IMAGE` / `LOAD_DELETE` | an asset's read        | yes      | opened G→H                    | once                   | next turn               | XTP, a body, an image | per key | a domain key and a body or image number       | as above                   | yes; `LOAD_DELETE` is the read gone without data                          |
| `SENSOR_CREATE` / `DELETE` (raised)                                  | watching GPS, compass  | yes      | opened G→H                    | once                   | not answered            | an enum               | —       | an enum (`Sensor`)                            | open / close               | yes, opened by the guest                                                  |
| `GPU_CREATE` / `DELETE`                                              | the GPU                | yes      | H→G news                      | once                   | next turn               | scalars               | —       | none, there is one                            | open / close               | yes, news of a device the host opened, on the page and on the device      |
| `BODY_READ` (host stream)                                            | a body the host holds  | yes      | G→H                           | once                   | between turns           | a byte span           | —       | a body number                                 | read and close             | yes; a length of 0 closes alone                                           |
| `STORAGE_SET` (host stream), stored values in `WORLD_CREATE`         | storage by key         | lasting  | written G→H, read at boot H→G | repeated               | between turns           | a UTF-8 span          | per key | a string key (`pxl:settings`)                 | write / read at boot       | a file in all but name; the key is a path                                 |
| `SETTING_UPDATE` / `TOGGLE`                                          | a setting              | lasting  | H→G                           | repeated               | next turn               | scalars               | per key | a key (u32)                                   | write / ctl                | yes, as a sysctl                                                          |

### Events and streams

| Command                                     | Way | How often       | Order  | What matters               | Candidate                    | Fits                       |
| ------------------------------------------- | --- | --------------- | ------ | -------------------------- | ---------------------------- | -------------------------- |
| `INPUT_POINTER`, `INPUT_WHEEL`              | H→G | continuous      | strict | every event                | a queue, WASI's `stream<T>`  | yes                        |
| `SENSOR_UPDATE`, `SENSOR_ORIENTATION`       | H→G | continuous      | yes    | mostly the latest          | a queue or a latest value    | half                       |
| `SENSOR_ERROR`, `SENSOR_ORIENTATION_LOST`   | H→G | once            | —      | the end of the above       | a stream's end               | yes, on the stream         |
| `WORLD_SNAPSHOT` (raised)                   | G→H | when it changes | —      | the latest, replaced whole | published state and a notice | not as a lossless stream   |
| `FETCH_UPDATE`, `LOAD_UPDATE`, `LOAD_IMAGE` | H→G | once per open   | —      | the result                 | a future                     | a completion, not a stream |

### Left out

| Command                                                                                   | Why                                                           |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| `WORLD_CREATE`                                                                            | the kernel's start, not a resource                            |
| `WORLD_UPDATE`                                                                            | the frame's boundary, part of the turn                        |
| `PROFILER_START` / `STOP` / `LOG` / `READ` / `TAKE`                                       | diagnostics: queries and switches, a reply that is an address |
| `NOP`                                                                                     | the rings' own test                                           |
| `TEXT_LAYOUT`, `TEXT_RASTER` (host stream)                                                | a function of its arguments, with no lifetime                 |
| `PANIC` (host stream)                                                                     | a report before the trap, once                                |
| `INPUT_TRACKING` / `VIEW` / `RESET_DIST` / `COMPASS_RESET` / `GO_TO`, `CHAT_CREATE`       | the UI's intents: a press, a search result, a line said       |
| `SENSOR_OPEN` / `CLOSE` / `ENABLE_LOCATION` / `PERMISSION` / `VISIBILITY`, `INPUT_UPDATE` | the UI's lifecycle and policy, not a device's data            |

## What the table says

- **The common verbs cover about half.** Surfaces, reads, sensors, bodies,
  storage and settings open, are used and close — some twenty of forty-five
  commands and five stream operations. The rest is the frame's boundary, the
  UI's intents, calls without state and diagnostics.
- **Either side opens.** The host opens a surface and the guest issues its
  handle; the guest opens a read or a sensor and the host serves it. Unix has
  one side opening the other's files. A resource model here has both sides able
  to serve, which is Plan 9's 9P used both ways rather than a process's system
  calls.
- **Identity is not one kind, and should not be made one.** A handle the guest
  issues, the guest's own domain key, a number the host issues, an enum, a
  string. A fetch's key is what a read is deduplicated and canceled by; a
  descriptor in its place adds a table from key to descriptor and loses what the
  key meant.
- **The verbs are a subject added, changed and removed.** Asked for or reported,
  as a kernel's uevent says add, change and remove: `GPU_CREATE` says a device
  was added, `SENSOR_UPDATE` that a reading changed, `LOAD_DELETE` that a read
  went without data, so the guest asks again. Every command reads that way.
- **Three kinds of flow, not one.** A queue, where every item and its order
  matter (the pointer); a latest state, where the newest reading is what counts
  and a whole state is replaced (the snapshot, most of a sensor); and a future,
  one result for one request (a read). Called one `stream<T>`, the API reads
  better and the meaning goes. The snapshot is published state with a notice of
  change, which is what the change log was when it left the contract.
- **The guest asks through one channel, packed two ways.** Each side puts work
  in the other's queue and the other takes it on its turn, the host between
  turns. A guest's request is a message — four words and a packet — or a record
  in its stream — a few words, thousands to a turn for what one turn asks of the
  GPU. Whether an answer comes is the message's `ACKREQ`, not which place it
  went to.

## What follows

1. **No system-call or descriptor API now.**
2. **The minors are a subject added, changed and removed**, asked for or
   reported; a failure is the result's `-errno`. Written where the minors are
   defined (`core abi/cmd`), and no command needs renaming.
3. **One request channel each way.** The stream is the guest's requests packed
   for volume, not a second kind of request.
4. **Queue, latest state and future stay three things.** None is folded into one
   stream abstraction.
5. **No namespace.** A command's major already names a resource's class. A
   9P-style namespace — names to open, discovery, late binding — waits for a
   demand for one.

xpute's own "stream", a turn's calls laid end to end, is a different thing from
any of the three flows and keeps its name; renaming it waits until a flow
abstraction is shown to recur as a thing of its own.

## Where it stands

**Recorded** 09-30; **corrected** 10-01: the device's C calls are its host's own
services and left the table; the guest's requests are one channel packed two
ways; the minors' reading is written in `core abi/cmd` and no command needed
renaming. **Superseded** 10-01 in part: "no system-call or descriptor API now"
and "no namespace" gave way to a guest's files reached as Plan 9 reaches them
([20261001-a-guest-s-files-as-plan-9-s.md](./20261001-a-guest-s-files-as-plan-9-s.md)).
