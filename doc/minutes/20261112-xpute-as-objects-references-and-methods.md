# xpute as objects, references and methods

**2026-11-12** — recorded with the user. Main was at `0487291f80` when this was
written. It gathers the axes the user set for xpute's rework on the xpute-ring
line, what was decided along them, and the model the work is heading to. Who
decided is said where it was the user, or reduce-s23-light-p95 within what the
user gave it; the rest was proposed by the agents and taken by them.

## What xpute is

xpute exists to bind the native and the web platforms into one: a program's core
runs the same on a page and on a device. It is a foreign function interface
between the core and its runtime, not a virtual machine, a monitor or an
operating system, and no such analogy is kept in its names or its prose. It is to
be published, and is judged as if pixelet were only its first user. The rework
keeps it to the capability it already had and adds no feature. Its kit and its
C++ core are general tools and stay, whether or not pixelet calls them. C++ is
the user's to control: it catches up as a whole at the very end, with no C++ work
in between, while the C++ that the generator writes follows the spec.

## How the work is judged

The user holds every change to these, in this order:

- **Structural reduction first.** A change reduces the structure to what the
  contract means, with fewer concepts, less state and one owner for each fact. A
  patch laid over a part to make one case pass is refused. Where reduction is
  not possible within the piece at hand, the work stops and says so.
- **Symmetry by role and contract**, whether or not a side uses it today. A
  missing half is a fault. Rust, C++ and TypeScript are laid out alike, in
  places, names, what a module holds and the shape of generated code, and
  differ only where the language makes them (the code skill holds this).
- **Convention**: what Unix, WASI and WIT do, checked against them rather than
  assumed.
- **State space**: each change says what state it removed or added, and what an
  addition buys.
- **Names** by the naming skill, and by what a thing is.
- **Who decides**: an agent proposes and never decides on its own. The user
  decides, and reduce-s23-light-p95 decides within what the user gives it and
  takes work into main after its own review and the GPU checks.

## What was decided

- **The boundary.** The rings went. The core reaches its runtime through three
  host functions, `receive`, `send` and `call`, passing frames in its memory.
- **Calls.** Each side has the same three ways: `call` answers at once and is for
  a caller that may wait, which is the core inside its turn; `send` is
  fire-and-forget, or carries an optional completion handler, which makes it a
  request (in TypeScript an `(err, val)` callback; in the core a completion
  handler registered at boot, found by the handle `send` returns); `on`
  registers a side's own handlers once, at boot. The UI never waits, on any
  platform. A function in one direction needs no twin in the other; what is
  symmetric is the way.
- **The turn.** xpute fixes the turn's start and deadline, takes every waiting
  frame to its handler (`recv`, hidden from programs), then calls the
  program's turn function, which runs on every entry and answers the wake.
  SYS_TURN keeps its meaning, a turn the policy granted. A lost GPU reaches the
  core as a frame, GPU_LOST, ordered before the next GPU_READY.
- **The core's end of the channel** is `connection` (`conn` in snake_case
  names, with files and types left whole), the same name as the runtime's end.
  pixelet's own door goes: its UI protocol's end joins `ui`, its boot and turn
  function go to the crate's root, and `layout` stays at the top of the core's
  order with `.fixed`'s duplicate state removed.
- **Files.** A path is a tuple of tokens, each an i32 or a string, with no root,
  namespace or meaning given by xpute. The program gives one binding for every
  path.
- **Names and places.** The core's declaration is its `manifest`; the channel's
  modules are `io`; the golden reader is development code under `dev/`; the
  command number's layout belongs to the frame.

## The model ahead

Seen as objects, the three words in use, file, handle and command, are one model:

| In use | As objects |
| --- | --- |
| a file, a GPU object, the camera | an object |
| a handle, an `fd` | a reference to it |
| a command | a method called on it |
| a path | the object's name, looked up to a reference |
| open, close | taking a reference, letting it go |
| a protocol | the object's interface |
| `on` at boot | implementing its methods |
| a command with a prefix (`CAMERA_`) | a method of a singleton whose reference is known from the start |

Android's Binder carries calls this way: a handle, a method's number and its
arguments, as a frame carries a tag, a command and a packet.

The order from here:

1. Finish the dispatch change, the move to `connection`, pixelet's door, and the
   TypeScript end, the Rust runtime end and native.
2. The file resource: one WIT-shaped interface, `file`, with `open(path)`,
   `read` into the caller's buffer, `write`, close as letting the reference go,
   and `changed`. A write's completion says the resource took it, and close's
   says it is applied. Replacing a file moves from the runtimes into one helper
   the program's replace-kind resources share, atomically. A reload cancels
   every reference, and a resource's server aborts what was never closed.
   pixelet's paths are all files, and no general resource declaration is built.
3. Once that skeleton is fixed, and not before: whether the UI's commands become
   methods of singleton objects, grouped as interfaces in place of name
   prefixes; whether SYS and the GPU's control become files too; and whether
   `handle` and `fd` stay two words, as Linux keeps a file's `fd` apart from a
   GPU buffer's handle, or become one.
4. C++ catches up last.

Work reaches main by review and a fast-forward, and from then on everyone works
in the main checkout.
