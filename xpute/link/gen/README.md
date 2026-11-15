# xpute-link-gen

xpute-link-gen writes the link's bindings, in Rust, TypeScript and C++, from the
WIT that declares them. It translates as xpute-gen does, one concept for one
concept in each language's own form, and writes through xpute-gen's writers
([`gen`](../../gen/README.md)). What is its own is how WIT says which side
does what.

A package's world decides each side's role. The guest serves what its world
exports and calls what it imports; every other side, the TypeScript too,
serves the imports and calls the exports. So a package whose interfaces have
functions declares one world, and an interface that stands in none of it is
refused. A package that only gives the guest what it calls names its world
`imports`; the generator reads no world's name.

The bindings reach what they take from the kit and the link through one
module: in Rust and C++ a private `_rt` that re-exports only what the file
uses, as wit-bindgen's does, and in TypeScript named imports. Their own names are
fixed: `HANDLE`, `mount`, the serving trait `Guest` on the guest and `Host`
on any other side, a resource's `GuestDevice` or `HostDevice`, and the
internal `_Object`. A WIT name that meets one of them, or a name the bindings
import, takes `_`, as wit-bindgen's `Guest_` does.

## A program's bindings

A program declares one package, its world being its guest's, and asks for its
bindings on one side at a time. Each side's bindings are read over xpute's
own packages, which the program's WIT may take types of and its world imports
or exports: xpute:link's, then `wit/`'s, then the GPU's. The generator
carries those itself, their WIT embedded as it is built (`build.rs`, through
the links under `xpute/` here), and reads them in that order, with where each
side's bindings of them lie and the names a program depends on them by
(`TARGETS`, `PACKAGES`), so a program names only its own WIT and where its
own bindings go. The guest takes Rust and C++, and a host Rust and TypeScript. A host's
bindings, which serve the world's imports, carry `IMPORTS` and `LATER`; the
guest's carry neither.

The command is `xpute-link-gen`, installed once from this crate, from a
registry or from an xpute checkout, and it needs neither after. Its paths
are from the directory it runs in, the program's root, and each Rust file and
TypeScript directory lies in its crate or package there:

```sh
cargo install xpute-link-gen        # or: cargo install --path xpute/link/gen

# A Rust guest: a module per interface beside the file, and the file the
# package's root module.
xpute-link-gen guest wit --rs core/src/abi.rs

# A C++ guest: a header per interface, each including xpute's library as
# its amalgamation's one header, xpute.hpp.
xpute-link-gen guest wit --cpp cpp/app/abi

# A page's host: a module per interface, imports.ts and later.ts.
xpute-link-gen host wit --ts ui/src/abi

# A device's host: the Rust it serves on xpute-native-runtime's link. The UI
# on the device takes the page's TypeScript.
xpute-link-gen host wit --rs runtime/src/abi.rs
```

A side's Rust and TypeScript, or its Rust and C++, may go in one run. With
`--check` it writes nothing and fails where a file is not what the WIT writes.
The command calls one function, `wit_outputs`, which a program's own tool
may call instead to check the bindings with its other generated files, as
xpute's `cargo spec` calls it for each of xpute's packages.
