# Resources rather than abstractions

**Decided** 10-01, reviewed with GPT. The documents had introduced xpute as
the arrangement an operating system has with a process. They now introduce it
as giving a program resources rather than abstractions, with an exokernel and
its library operating system as the nearest precedent. This records why that
frame and not the two others weighed, and what it leaves open.

## The three frames

- **An operating system and a process.** The first introduction. It names
  who owns the machine, but a process is handed abstractions — files,
  virtual memory, threads — and the guest here is handed a region and a
  quota and builds its own: an allocator, a scheduler, paging, a file table.
- **A hypervisor and a unikernel.** Proposed and set aside. A hypervisor
  virtualizes: its guest believes it owns a machine. Nothing here is
  virtualized; the guest sees its real offsets and its real quota. And a
  hypervisor owns the scheduling policy, where here the guest does.
- **An exokernel and a library OS** (Engler, Kaashoek and O'Toole, 1995).
  Taken. The outer side grants concrete resources and keeps them apart; the
  abstractions and the policy over them live with the program. The outer
  quantum is a cooperative
  counterpart of the CPU time slice Aegis handed a program, which the
  program scheduled within.

The split is not "mechanism outside, policy inside": the allocator, the
scheduler and the file table are mechanisms, and they are the guest's. It is
**ownership and grant of the platform's resources outside, abstraction and
management inside.** The README's line that xpute carries the mechanism
and the program the policy is about another split, the library against the
program that links it, and stays.

## Where it is not an exokernel

- **It is not a protection boundary.** An exokernel's split is made for
  software it does not trust: it protects resources and lets applications
  manage them. Here the guest is trusted and cooperative — a WebAssembly
  guest's linear memory confines it as a side effect, a native one shares the
  process — and the contract assumes as much.
- **A handle is not a secure binding.** Its generation gives a name temporal
  validity — a handle kept past its release names nothing that took its slot
  — but it authorizes nothing, where an exokernel's secure binding checks a
  principal's right to a resource once, at binding, so each access after is
  cheap.
- **Nothing enforces the grant.** An exokernel ends a slice with a timer and
  revokes what a program will not give back. Here a turn ends when the guest
  returns, and a guest in a loop holds the host's thread. Between turns the
  host takes the time back simply by not ringing.
- **Nothing is revoked.** Memory is granted once and kept for the guest's
  life, and taken back whole when it ends; with one cooperative guest
  there is nothing for revocation to arbitrate, so its absence follows from
  the topology rather than being a gap.
- **I/O is not the lowest level there is.** An exokernel exposes disk blocks
  and packets. The host exposes the thinnest useful capability the platform
  gives — not the lowest one, which in a browser is not a single thing — and
  names it as Plan 9 names a file (20261001-a-guest-s-files-as-plan-9-s.md).
  The two lenses answer different questions: who owns and grants, and how
  what is granted is named.
- **The GPU on a device is not passed through.** In a page the guest's
  drawing calls are records the host runs; on a device the guest's linked
  backend calls the device's API itself. That is direct access by the guest's
  own code, not a device assigned through an IOMMU.

The guest is not required to be an operating system. pixelet's kernel is
one — its application and its library OS linked into one image, a
unikernel's shape — and the conformance guest is not, and both are guests.

## Open

- **Several guests.** A guest a worker, each with its fixed memory, passing
  buffers between them by moving ownership, is a composition the contract
  says nothing of. It is not what 2026-09-11 decided — one core, one thread,
  one memory, workers as helpers for pure work
  (20260911-single-thread-core-and-when-to-spawn-a-worker.md) — so taking it
  reopens that decision rather than reframing it. Where a guest offers a
  service with a file's shape, the host binding it into another's namespace
  is the Plan 9 form; bulk data between them moves by ownership. Xen's grant
  tables are the precedent once a page is shared rather than moved.
- **Sharing a thread, or a trust boundary.** Several guests do not by
  themselves need a turn taken back: a guest a worker runs apart from the
  others, and a worker can be ended. Where guests share one thread of
  execution, or come not to trust each other, what is missing first is
  isolation and a way to stop a guest that does not return; then revocation
  of what a host lends — turns, handles, buffers, perhaps regions.

## Where it stands

**Decided** 10-01. README.xpute.md and ARCHITECTURE.xpute.md open on
resources rather than abstractions; the turn names Aegis's slice; the
contract says its unit is one host and one guest. No code changed.
