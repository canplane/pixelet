# pixelet

**[▶ Play](https://plei.me)**

> _A droplet fades. A pixelet remains._

_Started on December 15, 2025._

**pixelet** is a location-based spatial world built over real-world geodata ([Overture Maps](https://overturemaps.org/) + [GEDTM30](https://doi.org/10.5281/zenodo.15689805) elevation) — terrain rendered as voxels, with a separate voxel-object layer for building on top of it.

<img alt="Voxel objects built on real-world terrain" src=".github/asset/screenshot.png" height="480">

The client is a small operating system in a browser tab. A Rust **kernel**, compiled to one WebAssembly module, holds the world, schedules its own work and draws through WebGPU; a TypeScript **shell** drives it the way a driver drives a device, and decides nothing about the world. The arrangement they run in — one fixed memory, ring buffers, a turn on a doorbell — is [xpute](./ARCHITECTURE.xpute.md), which is generic and knows nothing of any of this. What follows is pixelet's own.

- **Real ground, as blocks.** Overture geometry and GEDTM30 elevation are rasterized ahead of time into pages of 512² blocks, four channels of ground type and one elevation raster each. A page is fetched, read where it lands in memory, and kept only while the view is near it.
- **One coordinate, 96 bits of fixed point.** A position is two packed words — three integer fields for which block, three 8-bit fractions for where inside it — so it resolves to 1/256 of a block. The arithmetic is integer, so it is the same bits on every machine, and the X field is cyclic, so the world wraps at the longitude seam.
- **Terrain and what you build meet only when a chunk is built.** The chunk is Minecraft's, at 64 blocks. The ground is a dense raster for a whole page; placed objects are a sparse tree per 64³ band. They converge in one pipeline, and the mesher and the pointer's ray read the same spans out of it — so what is drawn is what is hit.
- **Running short of memory costs detail, not the frame.** Work off screen is reclaimed first, and if that is not enough the LOD ladder steps one tier coarser and climbs back when the pressure passes. The view gets blockier; the tab does not die.
- **Numbers, not strings.** Commands, catalogs, assets and messages are numbered once in JSON under `spec/`, and both languages are generated from it. Nothing is ever looked up by name.

---

## System Overview

A block on screen has come a long way: a provider's geometry, a bake, a fetch, a chunk build.

```text
                   Overture Maps · GEDTM30
                              │
                              ▼             offline, once per version
              db-geo (PostGIS) ──▶ pxl-geo-bake
                              │
                              ▼
                   a page of 512² blocks:
             four ground channels and elevation
─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┼ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─
                              ▼             fetched by version, read in place
                      a terrain record
                              │
the player's placements ──────┤
a sparse tree per band        ▼             one L2 chunk, 64 blocks
                terrain paint + object paint
                              │
                              ▼
                         column runs
                              │
               ┌──────────────┴─────────────────┐
               ▼                                ▼
      a mesh per LOD tier               the pointer's ray
               │                                │
               ▼                                ▼
     draw list ──▶ WebGPU          a hit, from the same runs
```

`pxl-geo-bake` bakes, `pxl-kernel` holds the pages and schedules the builds, `pxl-render` paints, scans and meshes, and `apps/web` hands the draw list to WebGPU.

Above the dashed line is served as files; below it runs in the tab, on the turn's quota.

The kernel decides what exists, what is on screen, at what detail, and what to draw. The shell forwards events, does the I/O the kernel names, renders the UI, and turns the renderer's calls into WebGPU. What it renders from is the kernel's snapshot: one JSON document of UI state, written after a turn that changed it. What each frame and task cost goes to a read-only ledger, which nothing that schedules is allowed to read.

Almost nothing is computed at request time. Pages and assets are baked ahead and served from the bucket under names that cannot come to mean something else — a page under its bake's version, an asset under a hash of its own bytes; the one exception is forward search, which queries a database when asked.

[ARCHITECTURE.md](./ARCHITECTURE.md) is how the application is built, in three parts: the world it addresses — the levels, the coordinate, where the ground comes from — then the client that holds it, then the engine that computes with it.

---

## Getting Started

> Requires [Deno](https://deno.land/) 2.x, a Rust toolchain with the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`), and Docker for the local databases. Every dev, build and check task builds the kernel module from `crates/` first.

```bash
git clone git@github.com:canplane/pixelet.git
cd pixelet

cp .env.example .env
cp apps/web/.env.example apps/web/.env

deno task dev
```

`dev` runs the client alone: every server route is proxied to the live deployment, so no Docker, database or server process is needed. That is the way in for most work, and the wrong way for bake work: a published tile prefix never changes, so the proxy would keep serving the tiles that are already live rather than the ones just baked. For that, start the databases and run the whole stack:

```bash
docker compose --env-file .env -f infra/local/docker-compose.yml up -d
deno task dev:local
```

| Task                                 | Description                                                           |
| ------------------------------------ | --------------------------------------------------------------------- |
| `deno task dev`                      | The client alone, every route served by the live deployment           |
| `deno task dev:local`                | The client and the three servers it talks to, on this machine         |
| `deno task dev:web`                  | The client alone, against those servers                               |
| `deno task dev:geo`                  | pxl-geo only (`/geo`)                                                 |
| `deno task dev:api`                  | pxl-api only (`/api`)                                                 |
| `deno task dev:tiles`                | The dev tile server, demand-paging each page out of the bake          |
| `deno task preview`                  | The production bundle, served locally, data from the live deployment  |
| `deno task build:web`                | Build the client                                                      |
| `deno task build:wasm`               | Build the kernel module into `apps/web/src/kernel/kernel.wasm`        |
| `deno task bake:{tiles,poi,geocode}` | db-geo into `./out/<kind>/<version>`                                  |
| `deno task spec`                     | Generate Rust and TypeScript from the JSON under `spec/`              |
| `deno task deploy`                   | Build the client and publish the Cloudflare Worker                    |
| `deno task smoke <url>`              | What a deployment must answer, against a running one                  |
| `deno task check`                    | The gate: format, types, clippy, and the files generated from `spec/` |
| `deno task test`                     | Every test, every workspace member                                    |
| `deno task lint` / `fmt`             | Lint / format                                                         |

Four tools are run by path rather than by task, since each belongs to the tree it reads:

| Run                                                 | What it does                                                   |
| --------------------------------------------------- | -------------------------------------------------------------- |
| `cargo run --manifest-path asset/script/Cargo.toml` | Hash `asset/` into `./out/asset`, regenerate the asset tables  |
| `./asset/script/download_texture_pack.rs`           | Normalize the texture pack's sources into 256² tiles           |
| `./asset/script/download_moon.rs`                   | Cut the Moon's near side from NASA SVS's map into `asset/sky/` |
| `infra/geo/**/script/*`                             | Fill db-geo from Overture and GEDTM30 — infra/geo/README.md §4 |

To measure the client, use `preview` (`localhost:4173`). It is the bundle as the deployment builds it, so the dev server's unbundled modules and hooks are gone. It is still a development build, so the ledger overlay is compiled in — a deployed build does not have it.

---

## What runs where

| Layer     | Technology                                                         | Where                                                                 |
| --------- | ------------------------------------------------------------------ | --------------------------------------------------------------------- |
| Shell     | TypeScript, React 19, Vite                                         | the browser, served by the Worker                                     |
| Kernel    | Rust → one WebAssembly module, no wasm-bindgen; `embassy-executor` | the browser, in 512 MiB on a phone and 1 GiB elsewhere, reserved once |
| Rendering | WebGPU (WGSL), driven from the kernel                              | the browser's GPU                                                     |
| Edge      | Cloudflare Worker + R2                                             | the client, the baked pages, the assets                               |
| Servers   | Rust: axum, tokio-postgres — one per URL root, one database each   | one VM behind Caddy                                                   |
| Databases | PostgreSQL: `db-main`, and `db-geo` with PostGIS                   | the same VM                                                           |
| Geodata   | Overture Maps + GEDTM30, ingested into db-geo, baked into pages    | offline, into R2                                                      |
| Workspace | Deno drives every task, the Rust ones included                     | —                                                                     |

```text
                every request
                      │
           one Worker, one origin
┌─────────────────────┴─────────────────┐
▼   answered here                       ▼   relayed to the VM
the client                              /geo/search ─▶ pxl-geo ─▶ db-geo
/geo/tiles   R2, by version                                     (PostGIS)
/asset       R2, by hashed path         /api        ─▶ pxl-api ─▶ db-main
filled by the bakes, offline
```

The three roots are written in one file, `spec/pxl/spec/route.json`, and every relay reads them from there. A green push to the `deploy` branch takes a build number, deploys whichever half its files touched, and smoke-tests the result; [infra/README.md](./infra/README.md) has the procedures by hand.

Two formats are this project's: **PVF3**, a packed fixed-point coordinate, and **XTP**, a relocatable binary tree read without copying. One piece of the kernel is a port rather than a dependency: evanw/buddy-malloc, under the heap, and [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md) carries what its licence asks.

---

## Repository Structure

```
.
├── apps/
│   └── web/                  # The shell, and the Cloudflare Worker that serves it (cloudflare-worker/)
│       └── src/
│           ├── kernel/       #   the kernel's driver: abi, control, io, mem, render, sched, shell
│           ├── app/ page/    #   the frame, the router, the pages
│           ├── feature/      #   a screen's own pieces: map, search, setting, character, dev
│           ├── catalog/      #   the data screens show (cells, assets, geocode)
│           ├── locale/       #   i18n catalogues, dev-only ones apart
│           ├── config/       #   deployment config and routes
│           └── shared/       #   shared UI kit and browser helpers
├── crates/                   # The Rust workspace, grouped by where a crate runs
│   ├── xpute/
│   │   ├── core/             #   xpute-core: collections, codecs, XTP, errno — no domain
│   │   └── runtime/          #   xpute-runtime: the rings, the turn, the memory a module is given
│   └── pxl/
│       ├── spec/             #   pxl-spec: the model both ends follow
│       ├── web/              #   what the browser runs
│       │   ├── render/       #     pxl-render: the view
│       │   ├── kernel/       #     pxl-kernel: the stores, the schedulers, the turn
│       │   └── wasm/         #     pxl-wasm: the one WebAssembly module
│       └── server/           #   pxl-server: what geo and api both need to start
│           ├── geo-spec/     #     pxl-geo-spec: geographic provider values (Overture today)
│           ├── geo/          #     pxl-geo: the world's data server (/geo)
│           ├── api/          #     pxl-api: the application server (/api)
│           └── geo-bake/     #     pxl-geo-bake: the three page bakes and the dev tile server
├── packages/                 # The TypeScript workspace packages
│   └── xpute/
│       ├── core/             #   @xpute/core: the same kit in TypeScript
│       └── runtime/          #   @xpute/runtime: the rings' host end and the reservation
├── spec/                     # JSON every language is generated from (script/spec.ts),
│                             #   grouped as crates/ is: xpute/, pxl/{spec, web, server}
├── asset/                    # Shipped models and texture pack (cube, decor, actor),
│                             #   imported by nothing; script/ is the ingest, its own crate
├── script/                   # spec and locale generators and checks, the deployment smoke
├── infra/                    # local and production compose files, and what fills db-geo:
│                             #   geo/overture/ and geo/elevation/, each with init/ and script/
└── issue/                    # Decisions and why, the backlog, experiment/ for measurements
```

A crate's root holds directories only, sorted by kind; `pxl-geo`, `pxl-api` and `pxl-geo-spec` are flat.

| Crate           | Directories                                                              |
| --------------- | ------------------------------------------------------------------------ |
| `xpute-core`    | `abi` `alloc` `codec` `collection` `math` `status` `wire`, plus `golden` |
| `xpute-runtime` | `abi` `io` `ipc` `mem` `sched`, plus `clock`, `global`                   |
| `pxl-spec`      | `abi` `asset` `bake` `cell` `space`, plus `route`                        |
| `pxl-render`    | `mesh` `asset` `look` `frame` `gpu`                                      |
| `pxl-kernel`    | `mem` `sched` `world` `layer` `io` `control` `shell`                     |
| `pxl-geo-bake`  | `terrain` `poi` `geocode` `page`, and `bin` beside them                  |

The casing in the tree is not mixed by accident. Rust, the xpute packages and the kernel's driver (`apps/web/src/kernel/`) share one `snake_case` vocabulary across the boundary: `len`, `u32`, `new_len`. The rest of the shell and the scripts are `camelCase`, as TypeScript is. A name that crosses — a snapshot field, an action, a catalog entry — keeps the kernel's.

---

## Learn More

[AGENTS.md](./AGENTS.md) is how to work in the tree.

Four files carry a `.xpute` infix: [README.xpute.md](./README.xpute.md) and [ARCHITECTURE.xpute.md](./ARCHITECTURE.xpute.md) — the system under the application, its memory, rings, turn, credits and wire formats — with `LICENSE.xpute` and `THIRD_PARTY_NOTICES.xpute.md` beside them. They belong to the xpute repository and live here because xpute is developed here. None of them names pixelet, which is what lets `script/export-xpute.ts` drop the infix and send them out as they are.

---

[github.com/canplane/pixelet](https://github.com/canplane/pixelet)
