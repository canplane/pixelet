# Third-party notices

No source here is copied from someone else's code except under xpute, the
system pixelet runs on, which is developed in this repository and keeps its
own record of what it ported: [THIRD_PARTY_NOTICES.xpute.md](./THIRD_PARTY_NOTICES.xpute.md).

This file covers copied source only. Crates and npm packages that are merely
depended on are not listed: `crates/Cargo.toml` and `apps/web/deno.jsonc`
name them, and each carries its own license where it is published.

Data sources have their own obligations and their own place:
[infra/README.md](./infra/README.md) §6 records them, and
`apps/web/src/feature/setting/section/attribution.tsx` is where the ones
that must reach a user are shown.
