# Third-party notices

One port in this repository is of someone else's code rather than our own,
made in two languages, and its licence asks for a notice to travel with the
copy. Each file carries it; this says where.

This file covers copied source only. Crates and npm packages that are merely
depended on are not listed: `crates/Cargo.toml` and `apps/web/deno.jsonc`
name them, and each carries its own licence where it is published.

## evanw/buddy-malloc — `crates/xpute/core/src/alloc/buddy_tree.rs`, `cpp/xpute/core/alloc/buddy_tree.hpp`

Both files are [evanw/buddy-malloc][buddy] (MIT) line for line, the Rust and
the C++ each in its language, and the notice is at the top of each.

[buddy]: https://github.com/evanw/buddy-malloc

Data sources have their own obligations and their own place:
[infra/README.md](./infra/README.md) §6 records them, and
`apps/web/src/feature/setting/section/attribution.tsx` is where the ones
that must reach a user are shown.
