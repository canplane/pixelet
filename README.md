# pixelet

**[▶ Play](https://plei.me)**

> _A droplet fades. A pixelet remains._

_Started on December 15, 2025._

**pixelet** is a location-based spatial world built over real-world geodata ([Overture Maps](https://overturemaps.org/) + [GEDTM30](https://doi.org/10.5281/zenodo.15689805) elevation) — terrain rendered as voxels, with a separate voxel-object layer for building on top of it.

<img alt="Voxel objects built on real-world terrain" src=".github/asset/screenshot.png" height="480">

The client is a small operating system. A Rust **kernel** holds the world, schedules its own work and draws; a TypeScript **shell** drives it the way a driver drives a device, and decides nothing about the world. The same two run in a browser tab and in a device app: in the tab the kernel is one WebAssembly module drawing through the page's WebGPU, and on a device it is linked into a React Native app and draws through wgpu. The arrangement they run in — one fixed memory, ring buffers, a turn on a doorbell — is [xpute](./ARCHITECTURE.xpute.md), which is generic and knows nothing of any of this. What follows is pixelet's own.

- **Real ground, as blocks.** Overture geometry and GEDTM30 elevation are rasterized ahead of time into pages of 512² blocks, four channels of ground type and one elevation raster each. A page is fetched into one of a fixed number of records in memory and kept there until a page the view wants needs the room.
- **One coordinate, 96 bits of fixed point.** A position is two packed words — three integer fields for which block, three 8-bit fractions for where inside it — so it resolves to 1/256 of a block. The arithmetic is integer, so it is the same bits on every machine, and the X field is cyclic, so the world wraps at the longitude seam.
- **The view is cubes about the eye.** What is drawn is decided from the eye alone, in three dimensions: a chunk is a cube of its level, 64 of its cells a side, and it is split into eight while its cells would cover too much of the screen. A cube with nothing in it is never made, so the air costs nothing, and everything a view asks for — pages fetched, chunks built, records given up — goes in one order out from the eye. Storage stays 2D; only the addresses are 3D.
- **Terrain and what you build meet only when a chunk is built.** The chunk is a cube, Minecraft's section at 64 blocks a side rather than 16. The ground is a dense raster for a whole page; placed objects are a sparse tree per 64³ band, and an object belongs to the cube its band is in. They converge in one pipeline, and the mesher and the pointer's ray read the same spans out of it — so what is drawn is what is hit.
- **Running short of memory costs detail, not the frame.** Work off screen is reclaimed first, and if that is not enough the view's criterion is pulled a level coarser and let out again when the pressure passes. The view gets blockier; the client does not die.
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
                              ▼             fetched by version, into a record
                      a terrain record
                              │
the player's placements ──────┤
a sparse tree per band        ▼             one L2 chunk, a 64³ cube
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

`pxl-geo-bake` bakes, `pxl-kernel` holds the pages and schedules the builds, and `pxl-render` paints, scans, meshes and draws: through the page's WebGPU in a tab, through wgpu on a device.

Above the dashed line is served as files; below it runs on the client, on the turn's quota.

The kernel decides what exists, what is on screen, at what detail, and what to draw. The shell forwards events, does the I/O the kernel names, renders the UI, and on a page turns the renderer's calls into WebGPU. What it renders from is the kernel's snapshot: one packet of UI state, written after a turn that changed it. What each frame and task cost goes to a read-only ledger, which nothing that schedules is allowed to read.

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
| `deno task build:native`             | Build the kernel for a device, the xcframework the app links          |
| `deno task dev:native`               | Metro, for the device app                                             |
| `deno task bake:{tiles,poi,geocode}` | db-geo into `./out/<kind>/<version>`                                  |
| `deno task ingest:asset`             | Hash `asset/` into `./out/asset`, regenerate the asset tables         |
| `deno task download:{pack,moon}`     | Normalize the texture pack's sources, or cut the Moon's near side     |
| `deno task bake:stars`               | The star catalog into the renderer's `bright-stars.bin`               |
| `deno task render:brand`             | The favicon, app icons and link preview from `apps/web/brand/`        |
| `deno task spec`                     | Generate Rust and TypeScript from the JSON under `spec/`              |
| `deno task deploy`                   | Build the client and publish the Cloudflare Worker                    |
| `deno task smoke <url>`              | What a deployment must answer, against a running one                  |
| `deno task check`                    | The gate: format, types, clippy, and the files generated from `spec/` |
| `deno task test`                     | Every test, every workspace member                                    |
| `deno task lint` / `fmt`             | Lint / format                                                         |

db-geo is filled by the scripts under `infra/geo/**/script/`, run by path as [infra/geo/README.md](./infra/geo/README.md) §4 says: they make a database's state rather than a file the tree keeps.

To measure the client, use `preview` (`localhost:4173`). It is the bundle as the deployment builds it, so the dev server's unbundled modules and hooks are gone. It is still a development build, so the ledger overlay is compiled in — a deployed build does not have it.

### On a device

`apps/native` is the same shell as a React Native app, through Expo, with the kernel linked in and every route from the live deployment. It needs Xcode and CocoaPods besides the above, Node for Expo's native build, and the iPhone's and the simulator's Rust targets, which `rust-toolchain.toml` installs.

```bash
deno task build:native     # the kernel, as the xcframework the app links
(cd apps/native && deno install && deno run -A npm:expo@57.0.25 prebuild --platform ios --no-install)
(cd apps/native/ios && pod install)
deno task dev:native       # Metro
```

`apps/native/ios/` is generated from `app.json` and the module under `modules/platform/`, and is not kept. `pod install` and Xcode both run Node: put it on `PATH` for the first, and name it in `ios/.xcode.env.local` (`export NODE_BINARY=…`) for the second. Then build `ios/pixelet.xcworkspace` from Xcode, with a team to sign with for a phone. The app is built for iOS alone: `modules/platform` has no Android half.

---

## What runs where

| Layer     | Technology                                                                                                               | Where                                                 |
| --------- | ------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------- |
| Shell     | TypeScript, React 19 in React Native: react-native-web and Vite on a page, Expo on a device                              | the browser, served by the Worker; the device app     |
| Kernel    | Rust, no wasm-bindgen; `embassy-executor`: one WebAssembly module on a page, a static library linked into the device app | 512 MiB on a phone and 1 GiB elsewhere, reserved once |
| Rendering | WebGPU (WGSL), driven from the kernel: the browser's, or wgpu over Metal                                                 | the device's GPU                                      |
| Edge      | Cloudflare Worker + R2                                                                                                   | the client, the baked pages, the assets               |
| Servers   | Rust: axum, tokio-postgres — one per URL root, one database each                                                         | one VM behind Caddy                                   |
| Databases | PostgreSQL: `db-main`, and `db-geo` with PostGIS                                                                         | the same VM                                           |
| Geodata   | Overture Maps + GEDTM30, ingested into db-geo, baked into pages                                                          | offline, into R2                                      |
| Workspace | Deno drives every task, the Rust ones included                                                                           | —                                                     |

```text
                every request
                      │
           one Worker, one origin
┌─────────────────────┴─────────────────┐
▼   answered here                       ▼   relayed to the VM
the client                              /geo/search ─▶ pxl-geo ─▶ db-geo
/tile        R2, by version                                     (PostGIS)
/asset       R2, by hashed path         /api        ─▶ pxl-api ─▶ db-main
filled by the bakes, offline
```

The four roots are written in one file, `spec/pxl/spec/route.json`, and every relay reads them from there. A green push to the `deploy` branch takes a build number, deploys whichever half its files touched, and smoke-tests the result; [infra/README.md](./infra/README.md) has the procedures by hand.

Two formats are this project's: **PVF3**, a packed fixed-point coordinate, and **XTP**, a relocatable binary tree read without copying. One piece of the kernel is a port rather than a dependency: evanw/buddy-malloc, under the heap, and [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md) carries what its license asks.

---

## Repository Structure

```
.
├── apps/
│   ├── web/                  # The shell, and the Cloudflare Worker that serves it (cloudflare-worker/)
│   │   └── src/
│   │       ├── kernel/       #   the kernel's driver: abi, control, io, mem, render, sched, shell
│   │       ├── platform/web/ #   what differs on a page, behind @platform/
│   │       ├── app/ page/    #   the frame, the router, the pages
│   │       ├── feature/      #   a screen's own pieces: map, search, setting, character, dev
│   │       ├── catalog/      #   the data screens show (cells, assets)
│   │       ├── locale/       #   i18n catalogs, dev-only ones apart
│   │       ├── config/       #   deployment config and routes
│   │       └── shared/       #   shared UI kit and browser helpers
│   └── native/               # The same shell on a device, through Expo and Metro
│       ├── src/platform/     #   what differs on a device, behind @platform/
│       └── modules/platform/ #   its native half: one module, the view, the linked kernel
├── crates/                   # The Rust workspace, grouped by where a crate runs
│   ├── xpute/
│   │   ├── core/             #   xpute-core: collections, codecs, XTP, errno — no domain
│   │   └── runtime/          #   xpute-runtime: the rings, the turn, the memory a module is given
│   └── pxl/
│       ├── spec/             #   pxl-spec: the model both ends follow
│       ├── web/              #   what the client runs
│       │   ├── render/       #     pxl-render: the view
│       │   ├── kernel/       #     pxl-kernel: the stores, the schedulers, the turn
│       │   ├── wasm/         #     pxl-wasm: the kernel as one WebAssembly module
│       │   └── native/       #     pxl-native: the kernel as a device app's library
│       ├── server/           #   pxl-server: what geo and api both need to start
│       │   ├── geo-spec/     #     pxl-geo-spec: geographic provider values (Overture today)
│       │   ├── geo/          #     pxl-geo: the world's data server (/geo)
│       │   ├── api/          #     pxl-api: the application server (/api)
│       │   └── geo-bake/     #     pxl-geo-bake: the three page bakes and the dev tile server
│       └── tool/             #   pxl-tool: the ingest and the bakes that write files the tree keeps
├── packages/                 # The TypeScript workspace packages
│   └── xpute/
│       ├── core/             #   @xpute/core: the same kit in TypeScript
│       └── runtime/          #   @xpute/runtime: the rings' host end and the reservation
├── spec/                     # JSON every language is generated from (pxl-tool),
│                             #   grouped as crates/ is: xpute/, pxl/{spec, web, server}
├── asset/                    # Shipped models and texture pack (cube, decor, actor),
│                             #   imported by nothing
├── script/                   # the spec generator, the tree's checks, the deployment smoke
├── infra/                    # local and production compose files, and what fills db-geo:
│                             #   geo/overture/ and geo/elevation/, each with init/ and script/
└── issue/                    # Decisions and why, the backlog, experiment/ for measurements
```

A crate's root holds its modules, the directories sorted by kind and the one-file modules after them; `pxl-geo`, `pxl-api` and `pxl-geo-spec` are flat.

| Crate           | Directories                                                                            |
| --------------- | -------------------------------------------------------------------------------------- |
| `xpute-core`    | `abi` `alloc` `codec` `collection` `math` `status` `wire`, plus `golden`               |
| `xpute-runtime` | `abi` `ipc` `mem` `sched`, plus `clock`, `global`, `io`                                |
| `pxl-spec`      | `asset` `bake` `cell` `space`, plus `place`, `route`                                   |
| `pxl-render`    | `mesh` `asset` `look` `frame` `gpu`                                                    |
| `pxl-kernel`    | `abi` `door` `mem` `sched` `world` `layer` `io` `control` `shell`, plus `host`, `text` |
| `pxl-geo-bake`  | `terrain` `geocode` `page` `demand`, plus `poi`, `pyramid`, and `bin`                  |

The casing in the tree is not mixed by accident. Rust, the xpute packages and the kernel's driver (`apps/web/src/kernel/`) share one `snake_case` vocabulary across the boundary: `len`, `u32`, `new_len`. The rest of the shell and the scripts are `camelCase`, as TypeScript is. A name that crosses — a snapshot field, a packet's field, a catalog entry — keeps the kernel's.

---

## Learn More

[AGENTS.md](./AGENTS.md) is how to work in the tree.

Four files carry a `.xpute` infix: [README.xpute.md](./README.xpute.md) and [ARCHITECTURE.xpute.md](./ARCHITECTURE.xpute.md) — the system under the application, its memory, rings, turn, credits and wire formats — with `LICENSE.xpute` and `THIRD_PARTY_NOTICES.xpute.md` beside them. They belong to the xpute repository and live here because xpute is developed here. None of them names pixelet, which is what lets `script/export-xpute.ts` drop the infix and send them out as they are.

---

[github.com/canplane/pixelet](https://github.com/canplane/pixelet)
