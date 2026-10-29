# Third-party notices

One port here is of someone else's code rather than our own, and its license
asks for a notice to travel with the copy; each file carries it, and this
records where it came from. xpute, the system pixelet runs on, is developed in
this repository and keeps its own record of what it ported:
[xpute/THIRD_PARTY_NOTICES.md](./xpute/THIRD_PARTY_NOTICES.md).

This file covers copied source only. Crates and npm packages that are merely
depended on are not listed: `rust/Cargo.toml` and the `package.json` files name
them, and each carries its own license where it is published.

Data sources have their own obligations and their own place:
[infra/README.md](./infra/README.md) §6 records them, and
`ts/client/src/shell/setting/info/attribution.tsx` is where the ones that must
reach a user are shown.

## louis-e/arnis — `rust/server/geo-bake/src/separation/solve.rs` and `solve/`, `rust/spec/src/procedural.rs`

How bridges and tunnels stand is [louis-e/arnis][arnis] at `c96872e`
(Apache-2.0), its [element processing][processing] for bridges, highways and
railways without the blocks it places
([decision](./doc/minutes/20261006-bridges-and-tunnels-as-arnis-solves-them.md)):
the bake's solve in `separation/solve.rs`, its bridges in `solve/bridge.rs` and
its tunnels in `solve/tunnel.rs`, each of whose heads carries the license
notice, and the piers in `procedural.rs`, which reach the client and are marked
where they begin. Both read Overture's fields for OSM's tags and take their
widths from the ground. Arnis's `NOTICE` asks that this be kept with them:

```text
Arnis
Copyright (c) 2022-2026 Louis Erbkamm (louis-e)

This product is developed and maintained by Louis Erbkamm (louis-e).
It is free and open source software, licensed under the Apache License,
Version 2.0. See the LICENSE file for the full terms.
```

The license is at <https://www.apache.org/licenses/LICENSE-2.0>, and the client
shows the notice under Settings, Attribution
(`ts/client/src/shell/setting/info/attribution.tsx`).

[arnis]: https://github.com/louis-e/arnis
[processing]: https://github.com/louis-e/arnis/tree/c96872e0fa21573402d1b731ea87f796e8caa1d2/src/element_processing
