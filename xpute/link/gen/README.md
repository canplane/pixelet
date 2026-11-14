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
