# Discussions

Design discussions, trade-off analyses, and the reasoning behind decisions that
aren't self-evident from the code.

This is deliberately **not** where reference documentation goes. The split:

| Kind                                   | Where                            | Example                                            |
| -------------------------------------- | -------------------------------- | -------------------------------------------------- |
| What the system _is_                   | `README.md`, `ARCHITECTURE.md`   | the L0–L5 spatial hierarchy                        |
| How a subsystem works                  | a `README.md` next to that code  | `packages/.../pvf3/README.md`                      |
| How to operate something               | a `README.md` next to that infra | `infra/geo/README.md`                              |
| **Why a decision went the way it did** | **here**                         | why terrain tiles are pre-baked to an object store |

A document belongs here when it would otherwise be lost: the alternatives that
were weighed and rejected, the numbers a choice was made on, the constraint that
made an obvious-looking approach wrong. Once a decision hardens into "just how
the system works", the _result_ moves into reference docs — but the reasoning
stays here, because reference docs are the wrong place to explain a road not
taken.

Files are named `YYYYMMDD-topic.md`, matching the dating convention the
`checkpoint/YYYYMMDD-*` branches already use. The date is when the discussion
happened, not when it was last edited — a document can keep accruing follow-ups
under its original date.

**A superseded decision is not edited away.** This is the one convention worth
borrowing wholesale from ADRs (Architecture Decision Records), the most common
form of this practice elsewhere: when a decision is reversed, write a new
document and mark the old one as superseded by it, rather than rewriting the old
one to match the new answer. The reason the reversal happened — what was believed
at the time, what turned out to be false — is usually more valuable than either
decision on its own, and editing in place is exactly what destroys it.

Correcting a factual error, or updating measurements as better numbers arrive, is
ordinary editing and needs none of this — this convention applies to _conclusions_.

## Index

Newest first.

| Document                                                                                         | Topic                                                                                                                                                                          |
| ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [20260906-decor-lod-blend.md](./20260906-decor-lod-blend.md)                                     | Decor's density thinning now blends across LOD thresholds the same way cube content already did, via a new shared visibility registry                                         |
| [20260906-decor-lit-shading.md](./20260906-decor-lit-shading.md)                                 | Decor's material switches from unlit to the same lit+rim-light treatment terrain/objects get; a settings-store import had to move up a layer to keep `deno test` working       |
| [20260905-terrain-decor-scheduling-merged.md](./20260905-terrain-decor-scheduling-merged.md)     | Same-day follow-up: terrain and decor now share one build step/one ready flag, fixing the decor-renders-before-terrain desync                                                  |
| [20260905-tier1-geometry-baking-implemented.md](./20260905-tier1-geometry-baking-implemented.md) | Implements the 20260903 render-unification decision: terrain/objects/decor all bake real per-chunk geometry now, no more InstancedMesh; what changed vs. what's still deferred |
| [20260830-cozy-texture-pack-side-audit.md](./20260830-cozy-texture-pack-side-audit.md)           | Cozy texture pack side-face audit: BARE/SPORTS/HELIPAD mismatches found and fixed same day; 4 land-use types still unaudited                                                   |
| [20260817-object-placement-reach-cap.md](./20260817-object-placement-reach-cap.md)               | Object placement silently failing at elevation — reach-cap hypothesis ruled out, write path traced clean, root cause still open                                                |
| [20260817-lod-hysteresis-dead-zone.md](./20260817-lod-hysteresis-dead-zone.md)                   | Why LOD sharpens/coarsens at different distances, why the 1536 threshold's dead zone is now visible after FOG_FAR doubled, and why it isn't a performance issue                |
| [20260816-deployment-hosting.md](./20260816-deployment-hosting.md)                               | Serverless live-demo deployment: pre-baked tiles, object storage, hosting/cost comparison                                                                                      |
| [20260816-csr-vs-ssr.md](./20260816-csr-vs-ssr.md)                                               | Why the client stays Vite + client-rendered rather than moving to Next.js/SSR                                                                                                  |
| [20260811-coordinate-system.md](./20260811-coordinate-system.md)                                 | Why PVF3, the L4 anchor registry, and the three-tier camera frame are built the way they are                                                                                   |
| [20260810-mobile-and-perf-targets.md](./20260810-mobile-and-perf-targets.md)                     | Capacitor over React Native, and the two measured performance bottlenecks                                                                                                      |
| [20260708-concurrency-model.md](./20260708-concurrency-model.md)                                 | Why WASM + SharedArrayBuffer + preemptive threading was abandoned, and what a restart should look like                                                                         |
| [20260708-vision-and-scope-pivot.md](./20260708-vision-and-scope-pivot.md)                       | The original P2P/BFT metaverse target, why its scope collapsed, and what was kept                                                                                              |
