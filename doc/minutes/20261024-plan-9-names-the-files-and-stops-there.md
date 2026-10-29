# Plan 9 names the files, and stops there

**2026-10-24** — Whether the boundary should go the rest of the way to Plan 9:
every resource a file, opened, read, written and closed below a namespace the
host composes. Decided not; what would reopen it is written down. No code
changed.

## What is already taken

Since 10-01 a guest's files are reached as Plan 9 reaches them
(xpute/ARCHITECTURE.md, "System calls and files"): the guest issues the
descriptor, a path is walked a name at a time below a root the program numbers,
and the host binds each root. The calls cross the way io_uring's do: a
submission ring and a completion ring, a turn's records at once, a value or
`-errno` back.

The two answer different questions. Plan 9 answers what a thing is called and
who decides what a process sees; io_uring answers what a crossing costs. So the
boundary takes one from each, and taking more of Plan 9 is a question about
names, not about how calls cross.

## What going further would take

Plan 9's text is a convention, not its design: its own `/dev/draw` is a binary
protocol written to a file. Without the text, three things are left: a name a
resource is found by, four verbs every resource shares, and a namespace a parent
composes. The question is which of pixelet's commands want them
(ts/client/src/abi/cmd.spec.ts `Command`).

| Kind                    | Commands                                                  | As a file                                                                       |
| ----------------------- | --------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Opened, used and closed | `SURFACE_*`, `PREVIEW_*`, `WORLD_VIEW`, `PROFILER_*`      | Fits, but a handle or the subject already names each; a path finds nothing more |
| The turn's edges        | `WORLD_BOOT`, `WORLD_FRAME`, `WORLD_VISIBILITY`           | A clock, not a file                                                             |
| What the UI asks        | `INPUT_*`, `SETTING_SET`, `CHAT_SEND`                     | A write to a `ctl` file: the same command with a path in front                  |
| Readings                | `SENSOR_POS`, `SENSOR_ORIENT`, `WORLD_SNAPSHOT`           | A `read`, which loses that each replaces the last                               |
| The GPU's calls         | the records of a stream (`ipc/stream`), thousands a turn  | A binary write to a file, which is what the stream already is                   |

So:

- **Nothing here would be text.** No person reads these resources, so a `ctl`
  string buys nothing a typed packet does not.
- **What it costs is the types.** The command tables are generated and checked
  on both sides; a write to a path is checked by neither. And the three flows
  that 09-30 kept apart — a queue, a latest state, a future — become one `read`.
- **What it buys is a name and a namespace.** A name finds a resource the
  subject of a command already finds. A namespace pays only when one resource
  has to be seen two ways, and nothing here is yet.

## What would reopen it

- **A second guest** that must see a resource differently from the first.
- **A host that is far away**: a kernel run on a server, or a device reading
  another's sensors. Reaching a remote resource by the same calls as a local one
  is what a namespace is for.
- **Backends swapped often**, in tests or a replay, enough that binding a root
  wants to reach past files.

## A candidate, not taken

A read-only tree of the kernel's state — the turn, the handle tables, the open
descriptors — at a root of its own, read by the same `openat` and `read`. It
leaves the hot path alone, and it is the one place where text would suit, since
a person reads it. The profiler's counters are already read where they lie
(`PROFILER_READ`, `PROFILER_RINGS`), so the tree would be for the rest. Taken
when a tool asks for it.

## Where it stands

**Decided** 10-24: files by Plan 9's names; everything else as commands across
the rings. A namespace waits for one of the demands above, as
[20260930-resources-files-and-streams.md](./archive/20260930-resources-files-and-streams.md)
had it wait for a demand it did not name; that minute is archived, so this one
carries the condition.
