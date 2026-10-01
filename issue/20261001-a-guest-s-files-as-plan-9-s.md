# A guest's files, as Plan 9's

**2026-10-01** — xpute's system calls were xpute-shaped: read a body the host
holds by a number a command handed over, write a file whole, and a fetch as
three commands of its own. This decides the shape they take instead — a
guest's files, named and opened the way Plan 9 names and opens them, with
POSIX's and WASI's words for the calls — and what of Plan 9 is not taken.
No code changed; the change waits until this branch and
`observation-paging` are both on main, since it rewrites the reads that one
also changed.

## Why Plan 9 is the reference

Plan 9 is Unix's design carried to where its authors meant it to go: every
resource a file, one protocol (9P) to reach any of them, and a namespace
per process that its parent composes. The relevant part for xpute is not
"everything is a file" but how a file is reached: a client names the
handle, starts from a root it was given, walks down by relative names, and
ends the handle's life by letting go of it. That is already most of what
the boundary does, so taking it as the reference fixes the shape rather than
adding one.

| 9P                                                              | Here                                                                                 |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| the client chooses the fid (`Twalk`'s newfid)                   | the guest issues the descriptor, a generational handle (C6)                          |
| `Tattach` gives a root fid; `Twalk` names below it              | a root the program numbers, and `openat(root, path, flags)` below it                 |
| a process's namespace, composed by its parent (`bind`, `mount`) | the host binds each root to a backend: platform storage, a bake's origin, the assets |
| `Tclunk`                                                        | `close`                                                                              |
| a tag per request, `Tflush` to cancel one                       | only `open` waits, so the descriptor is its tag and `close` its flush                |
| `Tread(fid, offset, count)`                                     | `read(fd, dst)`, the file whole                                                      |

The calls keep POSIX's and WASI's names, since those are what a reader
outside knows; the structure is 9P's.

## The shape

- **The guest issues the descriptor.** Its handle table numbers it and the
  host holds what it names, as for every handle already (`abi/handle.rs`).
  Every record and every completion carries the whole handle, generation
  included, and the guest drops a completion whose generation is not
  current. So a slot is reusable the moment it is closed, and a late answer
  to an open that was closed cannot reach whatever took the slot.
- **A root is a capability.** The program numbers its roots; a path is
  relative to one, and the host refuses one that leaves it (`..`). The
  string carries no authority.
- **Only `open` waits.** The host completes it once the file can be read
  whole, with one xpute command it submits, as a fetch's result comes
  today: the descriptor and its size, or `-errno` (no such file is
  `ENOENT`; a refusal keeps the protocol's number beside it, as `detail`).
  The completion ring stays the guest's to write.
- **`read` and `write` are done between turns.** The host holds the file
  whole by the time `open` completes, so a read is a copy and needs no
  completion; what it names is kept until the next turn. A write is held
  until `close`, which commits it.
- **`close` ends the descriptor, opened or not.** On one still opening it
  cancels, and nothing is said of that descriptor again.

A descriptor is free, opening or open on the guest's side, and nothing
else: there is no closing state, because the generation does what waiting
out a turn would.

## What it removes

The body numbers the host hands over (`Bodies`, op `READ`), the fetch's own
three commands, the objects blob carried in the boot's packet, and the
whole-file `WRITE`. A fetch, an asset model's read, the boot's stored
objects and the store's save become the same four calls on different roots.
The scheduler's attempt key stays the scheduler's; the reads map a
descriptor to it.

## What is not taken

- **Everything a file.** Plan 9 would make the GPU, the text calls and the
  sensors files driven by messages written to a `ctl` file. Even Plan 9's
  own `/dev/draw` is a typed binary protocol written to a file, which is
  what the command tables already are, with their code generated and
  checked on both sides. Those stay commands.
- **An image decoded by the host.** What comes back is a GPU handle, not
  bytes, so it stays the program's command.
- **A tag apart from the descriptor.** Needed once a read or a write itself
  waits — a stream, an object too large to hold whole. Not before.

## Review

A second review proposed that the host issue the handle, the guest choose
an operation id per request, and a `CANCEL` apart from `close`. Rejected: it
rests on "the owner issues the handle", which is the opposite of C6 and of
9P's fids, and costs a second number space and a verb for what one
descriptor does. Taken from it: the completion travels host to guest on the
submission side, and a path is `openat(root, path)` rather than a prefixed
string.

## Where it stands

**Decided** 10-01. It supersedes two conclusions of
[20260930-resources-files-and-streams.md](./20260930-resources-files-and-streams.md):
"no system-call or descriptor API now" and "no namespace". Queued in
[BACKLOG.md](./BACKLOG.md) until both branches are on main.
