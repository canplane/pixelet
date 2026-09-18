# pixelet

**[▶ Play](https://plei.me)**

> _A droplet fades. A pixelet remains._

_Started on December 15, 2025._

**pixelet** is a location-based spatial world built over real-world geodata ([Overture Maps](https://overturemaps.org/) + [GEDTM30](https://doi.org/10.5281/zenodo.15689805) elevation) — terrain rendered as voxels, with a separate voxel-object layer for building on top of it.

<img alt="Voxel objects built on real-world terrain" src=".github/asset/screenshot.png" height="480">

The client is a small operating system in a browser tab. A Rust **kernel**, compiled to one WebAssembly module, holds the world, schedules its own work and draws through WebGPU. A TypeScript **shell** drives it the way a driver drives a device: it hands over a fixed block of memory once, then talks to the kernel only through two ring buffers and a doorbell.

---

## Getting Started

> Requires [Deno](https://deno.land/) 2.x, a Rust toolchain with the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`; the client's core module is built from `crates/` by every dev, build and test task), and Docker (for the local Postgres databases).

```bash
git clone git@github.com:canplane/pixelet.git
cd pixelet

cp .env.example .env
cp apps/web/.env.example apps/web/.env

docker compose --env-file .env -f infra/local/docker-compose.yml up -d

deno task dev
```

| Task                                 | Description                                                          |
| ------------------------------------ | -------------------------------------------------------------------- |
| `deno task dev`                      | The client and the three servers it talks to                         |
| `deno task dev:web`                  | The client only                                                      |
| `deno task dev:geo`                  | pxl-geo only (`/geo`)                                                |
| `deno task dev:api`                  | pxl-api only (`/api`)                                                |
| `deno task dev:tiles`                | The dev tile server, baking each page on demand                      |
| `deno task dev:remote`               | The client only, every route served by the live deployment           |
| `deno task preview:remote`           | The production bundle, served locally, data from the live deployment |
| `deno task build:web`                | Build the client                                                     |
| `deno task build:wasm`               | Build the kernel module into `apps/web/src/kernel/kernel.wasm`       |
| `deno task bake:{tiles,poi,geocode}` | db-geo into `./out/<kind>/<version>`                                 |
| `deno task spec`                     | Generate Rust and TypeScript from the JSON under `spec/`             |
| `deno task deploy`                   | Build the client and publish the Cloudflare Worker                   |
| `deno task smoke <url>`              | What a deployment must answer, against a running one                 |
| `deno task check`                    | Type-check every workspace member                                    |
| `deno task test`                     | Run every test                                                       |
| `deno task lint` / `fmt`             | Lint / format                                                        |

Three more are run by path rather than by name, because each belongs to the
tree it reads rather than to the repository:

| Script                                  | What it does                                                   |
| --------------------------------------- | -------------------------------------------------------------- |
| `asset/script/ingest_asset.rs`          | Hash `asset/` into `./out/asset`, regenerate the asset tables  |
| `asset/script/download_texture_pack.rs` | Normalize the texture pack's sources into 256² tiles           |
| `infra/geo/**/script/*`                 | Fill db-geo from Overture and GEDTM30 — infra/geo/README.md §4 |

The client has its own tasks under `apps/web`, reached with
`deno task --cwd apps/web <name>`: `build`, `preview`, `check:bundle` and the
rest. The root tasks above wrap the ones worth a short name.

`dev:remote` needs no Docker, no database and no server process: every route
is proxied to the published deployment. It is the quickest way in, and
the wrong way to work on a bake, since a published tile prefix is frozen.
`preview:remote` is the same data under the built client at
`localhost:4173`: what to measure the client against, since the dev server's
unbundled modules and hooks are not in it — the ledger overlay is, being
compiled in for development only, so a reading there is DevTools' profile.

---

## Deployment

One Cloudflare Worker serves the built client and reads R2 for everything
baked — terrain, POI and geocode by version under `/geo/tiles/`, every model
and texture by content hash under `/asset/` — and reverse-proxies the rest of
`/geo/` and all of `/api/` to the origin. Every address is same-origin and
unrewritten, and the three roots come from one file (`spec/route.json`), so
adding a route is a change to the one server that answers it.

The origin is a single Hetzner VM: Caddy in front of `pxl-geo` (`/geo`, the
world's data) and `pxl-api` (`/api`, the application), each with a Postgres of
its own. Forward search is the only feature that needs any of it at request
time; everything else is baked offline and already in the bucket.

A green push to main takes a build number, publishes whichever half its
files reached, and smoke-tests it
(`.github/workflows/check-and-deploy.yml`).
[infra/README.md](./infra/README.md) has the procedures by hand, the routing
contract, and what to verify after each.

---

## Tech Stack

The monorepo, package management, and task execution are unified under Deno.
The web app uses Vite as its bundling/build entry; the servers are Rust binaries
and Deno drives their tasks.

| Layer               | Technology                                                                   |
| ------------------- | ---------------------------------------------------------------------------- |
| Runtime / Workspace | Deno + TypeScript                                                            |
| Bundler (web)       | Vite                                                                         |
| Web UI              | React 19                                                                     |
| Servers             | Rust: axum, tokio-postgres — one per URL root, one database each             |
| Kernel              | Rust → one WebAssembly module, no wasm-bindgen; `embassy-executor` for tasks |
| 3D Rendering        | WebGPU (WGSL), driven from the kernel                                        |
| Database            | PostgreSQL (main + geographic/PostGIS)                                       |
| Geodata             | Overture Maps + GEDTM30 (elevation)                                          |

Two formats are built for this project rather than adopted: PVF3, a packed fixed-point coordinate, and XTP, a relocatable binary tree whose typed-array leaves are read without copying. Every page the client receives is XTP, and so is every packet on the rings.

The floor is iOS 26 and an iPhone 11, with WebGPU only. There is no WebGL path.

**Naming:** Most identifiers use `snake_case`, the TypeScript included. React hooks, stores and components keep the ecosystem's own convention. The engine is data-oriented and shares its vocabulary with the Rust (`len`, `u32`, `new_len`), and `snake_case` keeps the two languages consistent at the boundary.

---

## Repository Structure

```
.
├── apps/
│   └── web/                  # The shell: Vite + React 19, and the Cloudflare Worker (worker/)
│       └── src/
│           ├── kernel/       #   the kernel's driver: memory, module, doorbell, signals
│           ├── device/       #   the WebGPU host the renderer calls
│           ├── app/ ui/      #   the frame, screens and controls
│           ├── catalog/      #   the data screens show (settings schema, variants, geocode)
│           ├── locale/       #   i18n catalogues, dev-only ones apart
│           ├── config/       #   deployment config and routes
│           ├── dev/          #   dev tools: the ledger overlay, test page
│           └── kit/          #   shared UI kit and browser helpers
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
│           ├── overture/     #     pxl-geo-spec: what Overture's own values mean
│           ├── geo/          #     pxl-geo: the world's data server (/geo)
│           ├── api/          #     pxl-api: the application server (/api)
│           └── geo-bake/     #     pxl-geo-bake: the three page bakes and the dev tile server
├── packages/                 # The TypeScript workspace packages
│   └── xpute/
│       ├── core/             #   @xpute/core: the same kit in TypeScript
│       └── runtime/          #   @xpute/runtime: the rings' host end, the strobe, the reservation
├── spec/                     # JSON every language is generated from (script/spec.ts):
│                             #   pxl-spec's at the top; kernel/, render/, xpute/ below
├── asset/                    # Shipped models and texture pack (cube, decor, actor),
│                             #   imported by nothing — asset/README.md
├── script/                   # spec and locale generators and checks, the deployment smoke
├── infra/                    # local and production compose files, and what fills db-geo:
│                             #   geo/overture/ and geo/elevation/, each with init/ and script/
└── issue/                    # Decisions and why, the backlog, experiment/ for measurements
```

Inside the Rust crates, a crate's root holds only its directories, sorted by
kind. `pxl-geo`, `pxl-api` and `pxl-geo-spec` are flat, being five or six
files each.

| Crate           | Directories                                                    |
| --------------- | -------------------------------------------------------------- |
| `xpute-core`    | `ds` `io` `kit`, plus `cmd`, `errno`, `error`, `log`, `vector` |
| `xpute-runtime` | `ipc` `sched` `sys`                                            |
| `pxl-spec`      | `asset` `bake` `cell` `space`, plus `cmd`, `fetch`, `message`  |
| `pxl-render`    | `mesh` `asset` `look` `frame` `gpu`                            |
| `pxl-kernel`    | `sys` `sched` `world` `layer` `io` `control` `shell`           |
| `pxl-geo-bake`  | `terrain` `poi` `geocode` `page`, and `bin` beside them        |

---

## World Model

Storage, transfer and rendering use different scales on one hierarchy ([ARCHITECTURE.md](./ARCHITECTURE.md#spatial-hierarchy)):

| Level | Name   | Extent       | Role                                                            |
| ----- | ------ | ------------ | --------------------------------------------------------------- |
| L1    | Block  | 1 unit       | The canonical cell, about 2.39 m at the equator                 |
| L2    | Chunk  | 64 blocks    | Meshing, LOD, raycasting and culling; composed in `64³` Z bands |
| L3    | Page   | 1024 blocks  | Terrain's streaming and storage unit                            |
| L4    | Anchor | 4096 blocks  | The floating origin the view is placed against                  |
| L5    | Shard  | 65536 blocks | Reserved                                                        |

- **Terrain** is dense 2.5D data: each L3 page carries four ground channels (land, water, structure, transport) and an elevation raster sampled bicubically.
- **Placed objects** are sparse 3D data: a palette per chunk and a sparse octree per `64³` band. They are client-local for now.
- **One pipeline for both.** At build time both paint into one temporary signed-Z volume, which is scanned into column runs and baked into per-tier meshes. The raycast walks the same runs, so what is drawn and what is clickable cannot drift apart.
- **Dense volumes are temporary.** They exist only while composing. What survives is the column runs and the meshes, retained under a byte budget. When memory runs short, the LOD gets coarser.

---

## Learn More

[ARCHITECTURE.md](./ARCHITECTURE.md) covers the runtime (the turn, the rings, the memory) and the engine (coordinates, the chunk pipeline, LOD, rendering, catalogs and wire formats) in detail.

---

[github.com/canplane/pixelet](https://github.com/canplane/pixelet)
