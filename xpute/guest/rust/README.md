# xpute-guest

xpute-guest is the guest's end of xpute in Rust: its end of the link to its
host, the clocks it reads, its log records and the report of its failures. A
program links it into its guest, which it builds as a WebAssembly module for
the web (`wasm32-unknown-unknown`) and as a library of a native crate.

The guest's three exports are defined by one macro, `link!`, given four things.
The first is a frame's cap in bytes, a multiple of 8 that holds a frame's words
(`HDR_SZ`) and the largest packet the program lays or takes. The second is its
boot, which mounts what it implements and opens what it calls. The last two are
the program's start and end of every entry: `enter` runs as each entry begins,
and `end` as it ends, answering whether it left work, in which case the entry
runs it again. A library exports the three as C symbols, which catch the
guest's panic and answer a trap.

A module is the guest's crate built as a `cdylib`, which exports its memory
and the three by name and imports `link_out` from `env`:

```sh
cargo rustc --crate-type cdylib --release --target wasm32-unknown-unknown
```

A native app's build makes the library (`expo/README.md`).

From the first export on, the guest's own bug or panic is its stop, told to the
host before it stops (`stop::hook`); one in `link_init` itself, before the link
is laid, is told by the trap alone. Natively the hook is the process's, so a
panic that is not the guest's own, another thread's or the host's while the
guest is out on `link_out`, goes on to the runtime's hook.

Here the guest implements xpute's `life`, which is called as the app is hidden
and shown and as a UI attaches, and has no work of its own to start or end an
entry with:

```rust
use std::rc::Rc;
use xpute_guest::abi::life;
use xpute_kit::status::errno::Errno;

struct Program;

impl life::Guest for Program {
    fn hidden(&self) -> Result<(), Errno> { /* save */ Ok(()) }
    fn shown(&self) -> Result<(), Errno> { Ok(()) }
    fn attached(&self) -> Result<(), Errno> { /* tell the UI where all stands */ Ok(()) }
}

xpute_guest::link!(4096, || life::mount(Rc::new(Program)), || {}, || false);
```

The bindings of xpute's own packages on the guest's side are `abi`, generated
from their WIT; the program's are written beside its code by
`xpute-link-gen guest`. The guest's memory and its global allocator are the
program's, declared as any Rust program declares them; the kit's heap over a
range (`xpute_kit::alloc::heap::HeapMalloc`, a `GlobalAlloc`) is one a program
may pick.
