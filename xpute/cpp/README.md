# xpute in C++

xpute's C++ is the kit, the link and the guest's end, under `xpute/`, beside
their Rust (`kit/rust`, `link/rust`, `guest/rust`). A C++ guest takes them
as two files, as SQLite ships its own: it includes `xpute.hpp` and compiles
`xpute.cpp` once. Its bindings, which
`xpute-link-gen guest --cpp` writes, include `xpute.hpp` as well, so the two
files and the bindings are all it compiles. `make -C cpp amalgamation` writes
the two files under `cpp/target/amalgamation/` (`amalgamation.awk`), and
xpute's own repository carries them at `cpp/amalgamation/`.

Everything is compiled as C++20 with no exceptions, no RTTI and no
floating-point contraction (`-std=c++20 -fno-exceptions -fno-rtti
-ffp-contract=off`). A fused multiply-add rounds once where Rust and
JavaScript round twice, so contraction would part the C++'s math from theirs.

A failed check in libc++'s headers is a bug on every target: `xpute.hpp`
opens by defining libc++'s `_LIBCPP_VERBOSE_ABORT`, so a file includes it
before any other header, or is compiled with `-include xpute.hpp`. A failed
check inside libc++'s compiled library is a bug in a module and aborts a
native guest's process.

## The guest's exports

`XPUTE_LINK(cap, boot, enter, end)` defines the guest's three exports, given
what Rust's `link!` is given (`guest/rust/README.md`). A module exports them by
name, and its first export runs its static initializers before anything of the
program. A library defines them as C symbols, which the runtime calls; they
catch a bug and answer a trap, and the runtime defines `link_out`, which the
guest calls.

## A module

A module is compiled with the WASI SDK's clang for `wasm32-wasip1` and linked
as a Rust guest's is, with no start files and no entry, and its stack first,
so that an overflow runs below address 0 and traps, of the size the program
chooses:

```sh
-nostartfiles -Wl,--no-entry -Wl,--stack-first -Wl,-z,stack-size=<bytes>
```

It then exports its memory and the link's three exports, and imports
`link_out` from `env` and nothing of WASI.
`make -C cpp module GUEST="<sources>" MODULE=<file> [STACK_SZ=<bytes>]` links
one so; `dev/guest.cpp`, the smallest guest, is what `make -C cpp wasm` links.

## Natively

A native app links its guest into a Rust crate, which holds the runtime
(`runtime/native/README.md`), so a C++ guest runs natively by the same rule. The
crate's build script compiles the guest's sources and the amalgamation's
`xpute.cpp` for the device, with the flags a module is compiled with, into a
static library the crate links, as the `cc` crate does:

```rust
// build.rs of the native crate.
fn main() {
    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .flag("-fno-exceptions")
        .flag("-fno-rtti")
        .flag("-ffp-contract=off")
        .include("xpute") // xpute.hpp
        .file("xpute/xpute.cpp")
        .file("guest/main.cpp")
        .compile("guest");
}
```

## Memory

The guest takes what it lays and lends, the link's boxes among it, from its
default memory resource (`std::pmr::get_default_resource`), which its program
sets, and its global `operator new` is the program's as well. The kit's heap
(`xpute/kit/alloc/heap.hpp`), a slab of size classes over a buddy allocator of
pages, is a `std::pmr::memory_resource` over a range the program gives it, for
a program that picks it.

## The tree

`dev/test/` is the tests' runner, golden reader and memory, never shipped.
`make -C cpp check` compiles every file on its own, `test` runs the tests and
links the amalgamation as a module, and `compile_flags.txt` gives an editor
the same flags.
