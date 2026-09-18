# pixelet

**[▶ Play](https://plei.me)**

> _A droplet fades. A pixelet remains._

_Started on December 15, 2025._

**pixelet** is a location-based spatial world built over real-world geodata ([Overture Maps](https://overturemaps.org/) + [GEDTM30](https://doi.org/10.5281/zenodo.15689805) elevation) — terrain rendered as voxels, with a separate voxel-object layer for building on top of it.

<img alt="Voxel objects built on real-world terrain" src=".github/asset/screenshot.png" height="480">

The client is a small operating system in a browser tab. A Rust **kernel**, compiled to one WebAssembly module, holds the world, schedules its own work and draws through WebGPU. A TypeScript **shell** drives it the way a driver drives a device: it hands over a fixed block of memory once, then talks to the kernel only through two ring buffers and a doorbell.

- **One fixed linear memory.** The shell reserves 512 MiB on a phone or 1 GiB elsewhere. Nothing grows it. The kernel lays it out the way a linker script lays out an image: a stack, the rings, the module's data, a resident terrain range, and one heap.
- **Rings and a doorbell.** Commands go in on a submission ring and results come back on a completion ring. The doorbell is one call, `interrupt(quota_ms)`, and its return value says when the kernel wants its next turn.
- **One cooperative scheduler.** A turn gets a quota sized from the display's measured frame. Inside it, one tick runs its phases in priority order, and async tasks yield when the quota is spent. A read-only ledger records where the time went.
- **Numbers, not strings.** Commands, catalogs, assets and messages are numbered in JSON under `spec/`. Rust and TypeScript are both generated from it, so the kernel never looks anything up by name.
- **Baked offline, streamed as pages.** The server rasterizes Overture geometry and elevation into 1024² pages in XTP, a zero-copy binary tree format. The client reads those pages where they land in memory.

---

## System Overview

```text
┌─ browser tab ─────────────────────────────────────────────────────────────┐
│                                                                           │
│  shell  apps/web  (TypeScript, React)                                     │
│    UI · input · sensors · HTTP · image decode · the WebGPU host           │
│    strobe: a turn per animation frame, only when something asked for one  │
│        │                                                   ▲              │
│        │ submit          interrupt(quota_ms) → wake_ms     │ complete     │
│        ▼                                                   │              │
│  ┌─ WebAssembly.Memory  (fixed, shared by both sides) ─────┴───────────┐  │
│  │   SQ ring ──▶  kernel  crates/pxl/wasm                  ──▶ CQ ring │  │
│  │                  world · chunks · camera · stores · ledger          │  │
│  │                  tick + tasks under the turn's quota                │  │
│  │                  view: meshes, draw list, shaders, renderer         │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
│        │ WebGPU calls (numbers and addresses)                             │
│        ▼                                                                  │
│      GPU                                                                  │
└───────────────────────────────────────────────────────────────────────────┘
          │ HTTP: XTP pages, content-addressed assets
          ▼
  Cloudflare Worker + R2 (the client, baked pages, assets)
          │ /geo/*  /api/*      (/geo/tiles/* and /asset/* never leave the edge)
          ▼
  Hetzner: Caddy ─┬─ pxl-geo (/geo)  ── PostGIS (Overture) ── offline bake, in pxl-geo-bake
                  └─ pxl-api (/api)  ── PostgreSQL
```

The kernel decides what exists, what is on screen, at what detail, and what to draw. The shell decides nothing about the world. It forwards events, runs the fetches the kernel names, decodes images, renders the UI from the kernel's snapshot, and turns the renderer's calls into WebGPU. The line between them is [issue/20260914-what-stays-in-javascript.md](./issue/20260914-what-stays-in-javascript.md).

### Packages and crates

The code is split two ways: generic kit (`xpute`) versus pixelet domain (`pxl`), and within `pxl`, model, view and kernel.

```text
               TypeScript                   Rust
             ┌──────────────┐           ┌──────────────┐
generic kit  │ @xpute/core  │  mirrors  │ xpute-core   │  no domain knowledge
             └──────┬───────┘           └──────┬───────┘
             ┌──────▼───────┐           ┌──────▼───────┐
model        │ @pxl/shared  │  mirrors  │ pxl-spec     │  space, cells, commands, bake formats
             └──┬────────┬──┘           └───┬──────┬───┘
                │        │        ┌─────────▼──┐   │
view            │        │        │ pxl-render │   │  meshes, glTF, LOD, draw list, GPU
                │        │        └─────────┬──┘   │
                │        │        ┌─────────▼──┐   │
kernel          │        │        │ pxl-kernel │   │  stores, schedulers, the turn
                │        │        └─────────┬──┘   │
                │        │        ┌─────────▼──┐   │
module          │        │        │ pxl-wasm   │   │  boot block, rings, doorbell
                │        │        └─────────┬──┘   │
                │        │                  │  ┌───▼────────────┐
server          │        │                  │  │ pxl-geo-bake   │  the three offline bakes
                │        │                  │  └────────────────┘
                │        │                  │  ┌────────────────┐
                │        │                  │  │ pxl-geo  /geo  │  the world's data
                │        │                  │  │ pxl-api  /api  │  the application
                │        │                  │  └────────────────┘
             ┌──▼─────┐ ┌▼─────────┐        │      ▲
apps         │apps/api│ │ apps/web │◀───────┘      └── pxl-overture: what Overture means,
             └────────┘ └──────────┘  kernel.wasm      read by pxl-geo and pxl-geo-bake
```

- **`xpute`** is the runtime kit, meant to stay portable to other projects. It has collections, the allocators (a slab over a buddy), the rings, the change log, the three levels of the scheduler, and XTP. It has no coordinates, terrain or tiles.
- **`pxl-spec`** is the model that both the server and the client follow. It holds the PVF3 coordinate system, the spatial hierarchy, the cell catalogs, the command numbers and the bake's wire formats. `@pxl/shared` is its TypeScript counterpart, and the API bakes with it.
- **`pxl-render`** is the view: what the model looks like when drawn. It builds voxel volumes and meshes, loads glTF, runs the LOD ladder, packs the billboards' sheet, writes the draw list, and holds the WGSL shaders and the renderer. It knows the model and nothing of the kernel. The kernel hands it a frame.
- **`pxl-kernel`** is the kernel. It owns the chunk scheduler, the stores, the fetch queue, the camera, the environment and the turn. Nothing in it knows about WebAssembly or a browser, so it runs natively under `cargo test`.
- **`pxl-wasm`** is the only crate that knows it is WebAssembly. It installs the allocator and the executor, fixes the memory's front with linker flags, and exports `boot`, `interrupt` and the ledger's reads. It uses no wasm-bindgen.

### The turn

Scheduling works at three levels, the way an OS schedules processes, threads and time slices.

```text
host (shell)                                      guest (kernel)
────────────                                      ──────────────
requestAnimationFrame, only if asked
quantum: quota from the measured frame,
  interactive or batch, less last overrun
submit WORLD_UPDATE (viewport, gap, credits)
interrupt(quota_ms) ────── rising edge ─────────▶ drain the submission ring
                                                  poll tasks on the quota:
                                                    frame: effects → tick phases → draw
                                                           interaction · visible · content
                                                           · cosmetic · report
                                                    build, fetch, glTF, decor, save
                                                  post signals to the completion ring
read the completion ring ◀── falling edge ─────── return wake_ms:
schedule the next turn                              0 next frame · n ms · none
```

- **Quantum** (`xpute sched/quantum`): the host sizes one quota. It measures the display's frame as a low percentile of recent frame gaps, not an assumed 60 Hz. A turn counts as interactive when there was recent input or the guest asked for the very next frame. An overrun comes off the guest's own next quota.
- **Tick** (`xpute sched/tick`): the guest's phases run in a fixed order against that one budget, and the order is the priority. `interaction` is O(1) and unbudgeted. `content` is guaranteed a share, so a busy `visible` phase cannot starve it.
- **Pass** (`xpute sched/frame_budget`): a list of steps runs until the budget is spent. The first step that does work always runs, so work always moves forward even with no budget left.

Async work (chunk builds, fetch decoding, glTF parsing, saving) runs as tasks on an `embassy` executor that the module installs. A task yields when the quota is spent and resumes on the next rising edge. An idle world asks for no turn, and the shell rings for none.

### Rings

```text
descriptors (36 KiB)                          payloads (8 MiB)
┌──────────┬──────┬──────┬────────────┬─────┐  ┌────────┬────────┬─────┐
│ capacity │ head │ tail │ slot_bytes │ ... │  │ slot 0 │ slot 1 │ ... │  4 KiB each
├──────────┴──────┴──────┴────────────┴─────┤  └────▲───┴────────┴─────┘
│ msg 0: tag · cmd|flags · result · packet ─┼───────┘
│ msg 1: ...                                │
└───────────────────────────────────────────┘
```

Each ring is single-producer and single-consumer, with 2048 messages of four words. The producer writes only the tail and the consumer writes only the head, so no lock is needed. Message _n_ owns payload slot _n_, so a slot is free exactly when its message is consumed. A push can fail for only two reasons: the ring is full (`EAGAIN`, the producer waits) or the packet is larger than a slot (`EMSGSIZE`, whatever the ring's state). A packet is an XTP tree that the kernel reads where it lies. Anything larger than a page crosses by address. There are three rings: submission, completion, and a spill that the completion ring overflows into, which keeps order when the completion ring is full. The design follows virtio and AF_XDP ([issue/20260917-the-ring-a-slot-and-its-payload.md](./issue/20260917-the-ring-a-slot-and-its-payload.md)).

### Memory

```text
address          size                   what                          placed by
───────────────────────────────────────────────────────────────────────────────────
0                1 MiB                  .stack  (overflow wraps, traps) rustc, --stack-first
1 MiB            36 KiB × 3             .ring.desc  SQ · CQ · spill     build.rs
2 MiB            8 MiB × 3              .ring.slot                      build.rs
32 MiB           ~0.5 MiB               .rodata .data .bss              wasm-ld, --global-base
~32.5 MiB        196.43 MiB             .fixed.terrain                  the kernel
~229 MiB         283 MiB phone          .heap  slab over buddy          the allocator
                 795 MiB desktop
512 MiB / 1 GiB  end, never grown
```

- **`.fixed`** holds what is placed once and never freed, at a size known at compile time. Today that is the resident terrain: 7 × 7 pages, one record each. A page's record is its own coordinate modulo the axis, like a clipmap, so eviction needs no list and a page cannot be refused.
- **`.heap`** is everything else, served by one allocator: a slab of size classes over a buddy of 4 KiB pages. A `String`, a band's voxel lanes and a chunk's mesh all come from the same range. When the heap refuses a request, that work waits for a later turn and the view gets coarser rather than failing.
- **One file for the numbers.** `spec/kernel/sys/mem/mem.json` holds the numbers that the kernel's map, the linker flags in `build.rs`, and the shell's reservation all read. They are asserted at compile time and again at boot.

Everything the canvas reads each frame lives here as lanes (structure-of-arrays views): terrain pages, voxel bands, column runs, meshes, the chunk table, decor instances, the object store. What stays in the JavaScript heap is glue.

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

| Task                       | Description                                                          |
| -------------------------- | -------------------------------------------------------------------- |
| `deno task dev`            | Start web + api concurrently                                         |
| `deno task dev:web`        | Frontend only                                                        |
| `deno task dev:api`        | Backend only                                                         |
| `deno task dev:remote`     | Frontend only, served by the live deployment                         |
| `deno task preview:remote` | The production bundle, served locally, data from the live deployment |
| `deno task build:web`      | Build frontend                                                       |
| `deno task build:api`      | Build backend                                                        |
| `deno task build:wasm`     | Build the kernel module into `apps/web/src/kernel/kernel.wasm`       |
| `deno task bake:tiles`     | Bake terrain into `./out/terrain/<version>`                          |
| `deno task ingest:asset`   | Hash `asset/` into `./out/asset` and regenerate the asset table      |
| `deno task spec`           | Generate Rust and TypeScript from the JSON under `spec/`             |
| `deno task deploy`         | Build the client and publish the Cloudflare Worker                   |
| `deno task check`          | Type-check all packages                                              |
| `deno task test`           | Run all tests                                                        |
| `deno task lint`           | Lint                                                                 |
| `deno task fmt`            | Format                                                               |

`dev:remote` needs no Docker, no database and no API process: every server
route is proxied to the published deployment. It is the quickest way in, and
the wrong way to work on a bake, since a published tile prefix is frozen.
`preview:remote` is the same data under the built client at
`localhost:4173`: what to measure the client against, since the dev server's
unbundled modules and hooks are not in it — the ledger overlay is, being
compiled in for development only, so a reading there is DevTools' profile.

---

## Deployment

One Cloudflare Worker serves the built client, the baked pages and the
assets out of R2 — terrain, POI and geocode by version, every model and texture by
content hash — and a reverse proxy to the API, so everything the server
answers is same-origin under `/api`, and `deno task deploy` publishes all
three.

The API itself, forward search's only real dependency, runs on a single
Hetzner VM alongside its two Postgres instances and Caddy. Everything else
is baked offline.

[infra/README.md](./infra/README.md) has both procedures, the routing
contract between them, and what to verify after each.

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

The floor is iOS 26 and an iPhone 11, with WebGPU only. There is no WebGL path ([issue/20260912-why-the-destination-is-wasm.md](./issue/20260912-why-the-destination-is-wasm.md)).

**Naming:** Most identifiers use `snake_case`, the TypeScript included. React hooks, stores and components keep the ecosystem's own convention. The engine is data-oriented and shares its vocabulary with the Rust (`len`, `u32`, `new_len`), and `snake_case` keeps the two languages consistent at the boundary.

---

## Repository Structure

```
.
├── apps/
│   ├── web/                  # The shell: Vite + React 19, and the Cloudflare Worker (worker/)
│   │   └── src/
│   │       ├── kernel/       #   the kernel's driver: memory, module, doorbell, signals
│   │       ├── device/       #   the WebGPU host the renderer calls
│   │       ├── app/ ui/      #   the frame, screens and controls
│   │       ├── catalog/      #   the data screens show (settings schema, variants, geocode)
│   │       ├── locale/       #   i18n catalogues, dev-only ones apart
│   │       ├── config/       #   deployment config and routes
│   │       ├── dev/          #   dev tools: the ledger overlay, test page
│   │       └── kit/          #   shared UI kit and browser helpers
│   └── api/                  # the Overture/DEM ingests and the asset ingest — no server since 09-18
├── crates/                   # The Rust workspace
│   ├── xpute/core/           #   xpute-core: generic runtime kit
│   └── pxl/
│       ├── spec/             #   pxl-spec: the model
│       ├── render/           #   pxl-render: the view
│       ├── kernel/           #   pxl-kernel: the kernel
│       ├── wasm/             #   pxl-wasm: the one WebAssembly module
│       ├── overture/         #   pxl-overture: what Overture means, for both servers
│       ├── geo/              #   pxl-geo: the world's data server (/geo)
│       ├── geo-bake/         #   pxl-geo-bake: the offline bakes and the dev tile server
│       └── api/              #   pxl-api: the application server (/api)
├── packages/                 # The TypeScript workspace packages
│   ├── xpute/core/           #   @xpute/core: generic kit, XTP, rings, doorbell, strobe
│   └── pxl/shared/           #   @pxl/shared: PVF3, cells, bake formats, asset tables
├── spec/                     # JSON every language is generated from (script/spec.ts):
│                             #   pxl-spec's at the top; kernel/, render/, xpute/ below
├── asset/                    # Shipped models and texture pack (cube, decor, actor, brand);
│                             #   input to `deno task ingest:asset`, imported by nothing
├── script/                   # spec and locale generators and checks
├── infra/                    # local and production compose files, operations
└── issue/                    # Decisions and why, the backlog, experiment/ for measurements
```

Inside the Rust crates, a crate's root holds only its directories, sorted by kind:

| Crate        | Directories                                                   |
| ------------ | ------------------------------------------------------------- |
| `xpute-core` | `ds` `io` `ipc` `kit` `sched` `sys`                           |
| `pxl-spec`   | `asset` `bake` `cell` `space`, plus `cmd`, `fetch`, `message` |
| `pxl-render` | `mesh` `asset` `look` `frame` `gpu`                           |
| `pxl-kernel` | `sys` `sched` `world` `layer` `io` `control` `shell`          |

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

[ARCHITECTURE.md](./ARCHITECTURE.md) covers the runtime (the turn, the rings, the memory) and the engine (coordinates, the chunk pipeline, LOD, rendering, catalogs and wire formats) in detail. [issue/](./issue/) records why each decision went the way it did, and [issue/BACKLOG.md](./issue/BACKLOG.md) lists what is queued.

---

[github.com/canplane/pixelet](https://github.com/canplane/pixelet)
