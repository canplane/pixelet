# xpute-link-gen

xpute-link-gen writes the link's bindings, in Rust, TypeScript and C++, from the
WIT that declares them. It translates as xpute-gen does, one concept for one
concept in each language's own form, and writes through xpute-gen's writers
([`gen`](../../gen/README.md)). What is its own is how WIT says which side
does what.

A package's world decides each side's role. The guest implements what its world
exports and calls what it imports; every other side, the TypeScript too,
implements the imports and calls the exports. So a package whose interfaces have
functions declares one world, and an interface with functions or resources
that stands in none of it is refused. A package that only gives the guest what
it calls names its world `imports`; the generator reads no world's name.

The bindings reach what they take from the kit and the link through one module:
in Rust and C++ a private `_rt` that re-exports only what the file uses, as
wit-bindgen's does, and in TypeScript named imports. The names they give
beside WIT's are fixed, among them `HANDLE`, `mount` and the trait an
implementation implements, `Guest` on the guest and `Host` on any other side,
a resource's `GuestDevice` or `HostDevice`. A WIT name that meets one of them,
or in Rust and TypeScript a name the bindings import, takes `_`, as
wit-bindgen's `Guest_` does.

## What of WIT it takes

The link is declared in WIT and is not the component model: the generator takes
a part of WIT, refuses the rest by name, and reads some of it its own way.

| WIT | Here |
| --- | --- |
| `bool`, `u8` to `u64`, `s8` to `s64`, `f32`, `f64`, `string` | taken |
| `record`, `variant`, `enum`, a type alias | taken |
| `list<T>` | taken; a list of numbers crosses as its bytes, a typed array in TypeScript |
| `option<T>` | taken, but no option of an option: TypeScript's none is one null |
| `result<T, E>` | taken, either side empty; any failure outside its error case is the frame's status, an errno |
| `char`, `tuple`, `flags`, `future`, `stream`, `error-context` | refused |
| `func`, and a resource's method | a method answered inside the crossing |
| `async func` | a method answered later, by a frame of its own that carries the request's tag; the component model's async ABI is not used |
| a resource's constructor, static function, getter or setter | refused |
| `resource` | an object under a handle the implementing side issues: opened by a URI through the root's `open`, or returned by a function, and let go by the drop every handle has |
| `own<T>`, `borrow<T>` | one handle, a `u32`; the link tells them apart nowhere |
| an interface with functions | one object under a named handle, the hash of its package and name, called with no open |
| `world` | says which side implements what, and nothing else: one a package, its name unread, and the link's own crossings in none |
| `xpute:link/types` `lent` | a record to WIT; to the bindings a buffer of the caller's memory lent in place until the answer, one an async call |

An object holds at most 255 methods, the drop taking the last number. A
packet's layout is the Canonical ABI's but for the four departures
[ARCHITECTURE.md](../../ARCHITECTURE.md) gives in "The link".

xpute's own packages borrow WASI's shapes and implement none of WASI: a guest
imports nothing of it.

| Package | Against WASI |
| --- | --- |
| `xpute:clocks` | `wasi:clocks@0.3`'s monotonic and system clocks, cut to `now` and `wait-for` |
| `xpute:log` | `wasi:logging`'s shape: a level, a context and a message |
| `xpute:fs` | its own: a `file` opened by URI that reads and writes a lent buffer, with no descriptors, directories or streams; its `error-code` names its cases as `wasi:filesystem`'s or `wasi:sockets`' do, but for two of its own |
| `xpute:stop`, `xpute:life`, `xpute:link` | xpute's own |

## A program's bindings

A program declares one package, its world being its guest's, and asks for its
bindings on one side at a time. Each side's bindings are read over xpute's own
packages, which the program's WIT may take types of and its world imports or
exports: xpute:link's, then `wit/`'s. The generator carries those itself, their
WIT embedded as it is built (`build.rs`, through the links under `xpute/` here),
and reads them in that order, with where each side's bindings of them lie and
the names a program depends on them by (`TARGETS`, `PACKAGES`), so a program
names only its own WIT and where its own bindings go. The guest takes Rust and
C++, and a host Rust and TypeScript. A host's bindings, which implement the
world's imports, carry `IMPORTS` and `CALLS`; the guest's carry neither. The
Rust is written for a crate of edition 2024, its prelude and its style.

The command is `xpute-link-gen`, installed once from this crate, and it needs
no xpute checkout after. Its paths are from the directory it runs in, the
program's root, and each Rust file and TypeScript directory lies in its crate
or package there:

```sh
cargo install --path xpute/link/gen

# A Rust guest: a module per interface beside the file, and the file the
# package's root module.
xpute-link-gen guest wit --rs core/src/abi.rs

# A C++ guest: a header per interface, each including xpute's library as
# its amalgamation's one header, xpute.hpp.
xpute-link-gen guest wit --cpp cpp/app/abi

# The web host: a module per interface, imports.ts and calls.ts.
xpute-link-gen host wit --ts ui/src/abi

# The native host: the Rust it mounts on the Side its runtime makes and
# gives it. The native UI takes the web's TypeScript.
xpute-link-gen host wit --rs host/src/abi.rs

# A package over a plugin's: the plugin's WIT, and where its bindings of the
# same side lie, which the program's name.
xpute-link-gen guest wit --rs core/src/abi.rs --use gpu/wit --rs gpu/guest/src/abi.rs
```

A side's Rust and TypeScript, or its Rust and C++, may go in one run. With
`--check` it writes nothing and fails where a file is not what the WIT writes.
A plugin's package is written as a program's is, and each `--use` after
the program's paths names one the program's package takes, read after
xpute's, with the paths of its bindings of the same side: each named by the
manifest, Cargo.toml or package.json, above where its bindings lie. The
command calls one function, `wit_outputs`, which a program's own tool may
call instead to check the bindings with its other generated files. xpute's
`cargo spec` calls it for each of xpute's packages with no plugin (`&[]`); a
program that brings a plugin passes the plugin's packages as its last
argument.
