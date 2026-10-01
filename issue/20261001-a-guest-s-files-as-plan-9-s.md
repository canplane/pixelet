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

## The table is xpute's (10-01)

The table that issues a descriptor with who it is for, and finds the owner
an answer to an open belongs to — or none, for a descriptor closed — was the
kernel's. Nothing in it was pixelet's: a second guest that opens a file
needs the same table and the same rule for a stale answer, so it is
xpute-runtime's `ipc/file`, in Rust and C++ alike, over an owner type each
program gives. The kernel keeps its owners and where each answer goes.

## Where it stands

**Built** 10-01, once both branches were on main: `ipc/sys` in both
runtimes, the kernel's descriptor table (`io/file.rs`), its roots in
`host.json`, and the host's in `apps/web/src/kernel/host.ts`. An image still
comes back as a handle, by LOAD_START and LOAD_CANCEL. Since:

- A path is walked a name at a time on the host's end, and an empty name,
  `.` or `..` is refused (`EACCES`), so a root is the whole of what it
  grants.
- The settings are the kernel's: storage's file is handed over unread in
  WORLD_BOOT, as a process is handed its environment — the start is a
  handshake, not an open the boot would have to wait on — and every write
  after is the kernel's own, through these calls.
- An open's answer comes on some later turn, not the next one: a transport
  takes what it takes.
- The service has traces of its own, apart from the boundary's C0 to C7
  (`spec/xpute/conformance/sys/file.tsv`): a late open, an absent file, a
  read, a write committed at close, a close while opening that is never
  answered, a slot reused under a new generation before the old open's
  answer, and the walk.

- An image is a file like any asset (`a59f64e4`): opened below the assets'
  root, stopped by closing it, and decoded by a call of the kernel's own
  that takes the opened file's bytes where the host is, never copied into
  the kernel. Its own start, cancel, failure and answer commands, the
  shell's map of running reads and the host-issued image id went with it.
  What came of it is a shape for any resource: get it as a file, change it
  by an operation of its domain, use what that makes under a handle the
  kernel issued — `openat`, `IMAGE_DECODE`, `IMAGE_TEXTURE`.
- The tree was read again for a second way to name, keep, stop or carry one
  thing, and none was left: the GPU, the text calls and the sensors are
  other operations, the search box's request is the shell's own, and the
  rings and the directory are the boundary under all of it.

Where it differs from 9P — no offset on a read, a size in the open's
answer, images and the GPU as commands — it differs on purpose, and is not
changed for being different. **Decided** 10-01. It
supersedes two conclusions of
[20260930-resources-files-and-streams.md](./20260930-resources-files-and-streams.md):
"no system-call or descriptor API now" and "no namespace".
