# pixelet

**[▶ Play](https://plei.me)**

> _A droplet fades. A pixelet remains._

_Started on December 15, 2025._

**pixelet puts the real Earth into blocks — your street, its buildings, the river beside it, the hills behind it — and lets you build on it.**

<img alt="Voxel objects built on real-world terrain" src=".github/asset/screenshot.webp" width="100%">

The planet is too large to load or draw at once, so pixelet follows one rule: **draw what the eye can tell apart, and nothing else.** A city, a mountain range or a tower behind a ridge costs the cells it covers on the screen, not what it holds.

```text
asks                                      answers

the eye                                       ▲
  │   what can be told apart, cell by cell    │
  ▼                                           │
chunks                                        │
  │   the pieces of the world drawn           │
  ▼                                           │
query                                         │
  │   the one way to ask what is there        │
  ▼                                           │
layers                                        │
  │   terrain · object · place                │
  ▼                                           │
tiles ────────────────────────────────────────┘
      fetched only when the view lacks them
```

- **Real geography is baked ahead; what you build is live.** [Overture Maps](https://overturemaps.org/)' geometry, OpenStreetMap's coastline and [GEDTM30](https://doi.org/10.5281/zenodo.18887460)'s elevation are painted into tiles before anyone asks; what players place is kept as they place it, and the trees and cars the ground grows are worked out where they stand, never stored.
- **Cells express detail, chunks are drawn, tiles are fetched.** One cell of the eye's view is one thing drawn, a chunk is a cube of those cells, and a tile is what arrives when the view lacks it.
- **The same world runs in a browser and on a device**, through one Rust kernel drawing with WebGPU.

## The eye decides

The world has its own fixed lattice. The eye lays a second grid over it, cells of equal solid angle, so they grow with distance. Where the eye can tell more apart, chunks split; where it cannot, they fold into one thing drawn.

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

The agent instructions are [AGENTS.md](./AGENTS.md) and the procedures [.agents/skills/](./.agents/skills/), kept vendor-neutral. An assistant's own directory is gitignored; link it at them once per checkout:

```bash
mkdir -p .claude
ln -sfn ../AGENTS.md      .claude/CLAUDE.md
ln -sfn ../.agents/skills .claude/skills
```

A personal, uncommitted variant takes a `.local.` infix (`*.local.skill.md`, `.agents/skills/.local/`), which `.gitignore` keeps out.

The tiles come three ways, named as a bakery's bread is: delivered, from the oven, or off the shelf. `dev` has them delivered: every server route is proxied to the live deployment, so no Docker, database or server process is needed. That is the way in for most work, and the wrong way for bake work: a published tile prefix never changes, so the proxy would keep serving the tiles that are already live rather than the ones just baked. For that, start the databases and run the whole stack, which bakes each tile when it is first asked for:

```bash
docker compose --env-file .env -f infra/local/docker-compose.yml up -d
deno task dev:oven
```

`dev:shelf` is the same stack with the tiles taken from the last bakes under `./out` and nothing baked on demand: what a publish of them would show, before it is uploaded. `preview:oven` and `preview:shelf` are the two over the production bundle.

| Task                             | Description                                                                                                                                                                                                                                                       |
| -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `deno task dev`                  | The client alone, every route delivered by the live deployment                                                                                                                                                                                                    |
| `deno task dev:oven`             | The client and the three servers it talks to, each tile baked when first asked for                                                                                                                                                                                |
| `deno task dev:geo`              | pxl-geo only (`/geo`)                                                                                                                                                                                                                                             |
| `deno task dev:api`              | pxl-api only (`/api`)                                                                                                                                                                                                                                             |
| `deno task dev:shelf`            | `dev:oven`, with the tiles taken from the last bakes under `./out`                                                                                                                                                                                                |
| `deno task dev:tile`             | The dev tile server, baking each tile out of the source when missed                                                                                                                                                                                               |
| `deno task preview`              | The production bundle, served locally, data from the live deployment                                                                                                                                                                                              |
| `deno task preview:{oven,shelf}` | The production bundle over `dev:oven`'s or `dev:shelf`'s servers                                                                                                                                                                                                  |
| `deno task clear:oven`           | Empty `dev:oven`'s cache, after a re-ingest or a bake change                                                                                                                                                                                                      |
| `deno task clear:shelf [kind…]`  | Delete the named kinds' sets under `./out` (all three when none), to bake them from nothing                                                                                                                                                                       |
| `deno task build:wasm`           | Build the guest module into `apps/web/src/kernel/guest.wasm`                                                                                                                                                                                                      |
| `deno task dev:native`           | Metro, for the device app                                                                                                                                                                                                                                         |
| `deno task bake <kind> [args…]`  | Make a kind's data (`script/tool.ts`): `terrain`, `place`, `division` from db-geo into `./out/<kind>/`; `earth` from `infra/earth/` into `./out/earth/`; `star` into the renderer's `bright-stars.bin`; `geoid <egm2008-5.pgm>` into `spec/spec/space/geoid.json` |
| `deno task download <what>`      | Fetch a source a tool reads: `pack`, the texture pack's; `moon`; `earth`                                                                                                                                                                                          |
| `deno task ingest:asset`         | Hash `asset/content/` into `./out/asset`, compose the texture pack, regenerate the asset tables                                                                                                                                                                   |
| `deno task render:brand`         | The favicon, app icons and link preview from `apps/web/brand/`                                                                                                                                                                                                    |
| `deno task spec`                 | Generate Rust and TypeScript from the JSON under `spec/`                                                                                                                                                                                                          |
| `deno task usage`                | What the deployment has used of its free allowances, before a publish                                                                                                                                                                                             |
| `deno task deploy`               | Build the client and publish the Cloudflare Worker                                                                                                                                                                                                                |
| `deno task smoke <url>`          | What a deployment must answer, against a running one                                                                                                                                                                                                              |
| `deno task check`                | Everything: format, rules, types, clippy, the device, the C++, the files made from `spec/`                                                                                                                                                                        |
| `deno task check:ship`           | What a deploy waits on: what it ships type-checks, lints and matches `spec/`                                                                                                                                                                                      |
| `deno task check:diff`           | `check` over the working tree's changes                                                                                                                                                                                                                           |
| `deno task test`                 | Every test, every workspace member                                                                                                                                                                                                                                |
| `deno task lint` / `fmt`         | Lint / format                                                                                                                                                                                                                                                     |

db-geo is filled by each source's `ingest` under `infra/geo/`, run by path as [infra/geo/README.md](./infra/geo/README.md) says: they make a database's state rather than a file the tree keeps.

To measure the client, use `preview` (`localhost:4173`). It is the bundle as the deployment builds it, so the dev server's unbundled modules and hooks are gone. It is still a development build, so it offers the nightly settings a deployed build leaves out; the profiler's overlay, which reads the numbers, is in both.

### On a device

`apps/native` is the same shell as a React Native app, through Expo, with the kernel linked in and every route from the live deployment. Its native projects are generated by `expo prebuild` from `app.json`, the config plugins and the module under `modules/platform/`, and are not kept. The kernel is built with the app, by a step of its native build (`apps/native/script/build-native.ts`): a Debug build's with the kernel's dev settings, a Release build's without, as a deployed page's is. `deno task dev:native` runs Metro, which a Debug build loads its bundle from. Setting a machine up, building, installing and what has gone wrong are the [device-build skill](./.agents/skills/device-build/SKILL.md).

---

## What runs where

The client is a Rust **kernel** and the TypeScript around it. The kernel holds the world, schedules its own work and draws; its **host** gives it memory, I/O and the device, and its **shell** is the UI, which decides nothing about the world. In a browser tab the kernel runs in one WebAssembly module drawing through the page's WebGPU; on a device it is linked into a React Native app and draws through wgpu. The arrangement they run in — one fixed memory, ring buffers, a turn on a doorbell — is [xpute](./xpute/ARCHITECTURE.md), which knows nothing of any of this.

Almost nothing is computed at request time. Tiles and assets are baked ahead and served from the bucket under names that cannot come to mean something else — a tile under its bake's version, an asset under a hash of its own bytes; the one exception is forward search, which queries a database when asked. [ARCHITECTURE.md](./ARCHITECTURE.md) follows the chain above, part by part.

| Layer     | Technology                                                                                                                     | Where                                                 |
| --------- | ------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------- |
| Shell     | TypeScript, React 19 in React Native: react-native-web and Vite on a page, Expo on a device                                    | the browser, served by the Worker; the device app     |
| Kernel    | Rust, no wasm-bindgen; `embassy-executor`: in one WebAssembly module on a page, in a static library linked into the device app | 512 MiB on a phone and 1 GiB elsewhere, reserved once |
| Rendering | WebGPU (WGSL), driven from the kernel: the browser's, or wgpu over Metal on an iPhone and Vulkan on Android                    | the device's GPU                                      |
| Edge      | Cloudflare Worker + R2                                                                                                         | the client, the baked tiles, the assets               |
| Servers   | Rust: axum, tokio-postgres — one per URL root, one database each                                                               | one VM behind Caddy                                   |
| Databases | PostgreSQL: `db-main`, and `db-geo` with PostGIS                                                                               | the same VM                                           |
| Geodata   | Overture Maps + OSM land polygons + GEDTM30, ingested into db-geo, baked into tiles                                            | offline, into R2                                      |
| Workspace | Deno drives every task, the Rust ones included                                                                                 | —                                                     |

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

The four roots are written in one file, `spec/spec/route.json`, and every relay reads them from there. A green push to the `deploy` branch takes a build number, deploys whichever half its files touched, and smoke-tests the result; [infra/README.md](./infra/README.md) has the procedures by hand.

One format was made here: **XTP**, xpute's relocatable binary tree, read without copying.

---

## Repository Structure

The tree is grouped by where a thing runs, and the kernel's modules by where they stand on the chain.

```
.
├── apps/
│   ├── web/                  # The shell, and the Cloudflare Worker that serves it (cloudflare-worker/)
│   │   └── src/
│   │       ├── kernel/       #   the kernel's host: abi, control, io, mem, render, turn; the shell's end
│   │       ├── platform/web/ #   what differs on a page, behind @platform/
│   │       ├── app/ page/    #   the frame, the router, the pages
│   │       ├── feature/      #   a screen's own pieces
│   │       ├── catalog/      #   the data screens show (cells, assets)
│   │       ├── locale/       #   i18n catalogs, dev-only ones apart
│   │       ├── config/       #   deployment config and routes
│   │       └── shared/       #   shared UI kit and browser helpers
│   └── native/               # The same shell on a device, through Expo and Metro
│       ├── src/platform/     #   what differs on a device, behind @platform/
│       └── modules/platform/ #   its native half: one module, the view, the linked kernel
├── crates/                   # The Rust workspace, grouped by where a crate runs
│   ├── spec/                 #   pxl-spec: the model every crate shares
│   ├── client/               #   what the client runs
│   │   ├── render/           #     pxl-render: the view
│   │   └── kernel/           #     pxl-kernel: the kernel
│   ├── client-bracket/       #   where the client is plugged in to run, no domain
│   │   ├── wasm/             #     pxl-wasm: the guest as one WebAssembly module
│   │   └── native/           #     pxl-native: the guest as a device app's library
│   ├── server/               #   pxl-server: what geo and api both need to start
│   │   ├── geo-spec/         #     pxl-geo-spec: geographic provider values (Overture today)
│   │   ├── geo/              #     pxl-geo: the world's data server (/geo)
│   │   ├── api/              #     pxl-api: the application server (/api)
│   │   └── geo-bake/         #     pxl-geo-bake: the tile bakes and the dev tile server
│   └── tool/                 #   pxl-tool: the binaries that write files the tree keeps
├── spec/                     # JSON every language is generated from (pxl-tool),
│                             #   grouped as crates/ is: spec/, client/
├── xpute/                    # The system under it, its own repository's root and workspaces:
│                             #   rust/, ts/ and cpp/ by language: kit, guest, host; spec/ the contract
├── asset/                    # Shipped models, texture pack and sky, imported by nothing
├── script/                   # the tree's checks, the deployment smoke, the usage report
├── infra/                    # local and production compose files, and what fills db-geo
└── doc/                      # The record: backlog.md what is queued, minutes/ each meeting's
                              #   record (archive/ those retired), experiment/ measurements
```

**What shares `pxl-spec` is Rust, and what does not is free.** The kernel, the servers, the bakes and the asset ingest compile the model, so they are crates; the Overture and elevation ingests put a provider's data into Postgres sharing nothing with the model, so they are scripts under `infra/geo/`. The host and the shell are TypeScript because neither decides anything about the world; what they need of the model — catalogs, ABI enums, routes — is generated into `apps/web/src/**/*.spec.ts` from the same JSON the Rust is generated from.

| Crate / package | What it is                                                                                                                                       |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `pxl-spec`      | The model: the position, the zooms and lattices, the block catalog, the command numbers, the tile formats, the asset tables. One implementation. |
| `pxl-render`    | The view: the mesher, glTF, the badge sheet, the draw list, the shaders, the renderer. It knows no kernel.                                       |
| `pxl-kernel`    | The kernel: the chain from the scene down to the reads, the turn, the memory's tenants, the profiler.                                            |
| `pxl-wasm`      | The guest in a tab: one module that fixes the memory's front with linker flags, imports the clock and exports `interrupt`.                       |
| `pxl-native`    | The guest on a device: a static library that runs every call on a thread of its own and exports `pxl_interrupt` through the C ABI.               |
| `apps/web`      | The kernel's host and its shell, both apps'; and the Cloudflare Worker that serves it all.                                                       |
| `apps/native`   | The device app: the same host and shell run by Metro, what only a device does, and the native module that links the kernel.                      |
| `pxl-server`    | What both servers need before answering: the environment, the database pool, the gate over internal routes.                                      |
| `pxl-geo-spec`  | What the geo server and the bakes agree on about db-geo: table names, and what Overture's categories and divisions mean.                         |
| `pxl-geo`       | The world's server: forward search, the one route that queries a database at request time.                                                       |
| `pxl-api`       | The application's server.                                                                                                                        |
| `pxl-geo-bake`  | The bakes — terrain, place, division, earth — and, behind a feature, the dev tile server.                                                        |
| `pxl-tool`      | Hashes `asset/` into the bucket's shape and writes the asset tables; generates code from `spec/`; bakes the stars, the Moon, the brand.          |

A crate's root holds its modules, the directories sorted by kind and the one-file modules after them; `pxl-geo`, `pxl-api` and `pxl-geo-spec` are flat. The kernel's are named as the chain is:

| Where on the chain | `pxl-kernel` modules                                     |
| ------------------ | -------------------------------------------------------- |
| the top            | `turn` `door` `shell`                                    |
| over the view      | `control`                                                |
| the chain          | `scene` `chunk` `view` `query` `layer`                   |
| under everyone     | `io` `sched` `mem` `abi`                                 |
| leaves             | `setting` `model` `decor` `billboard` `param` `approach` |
| beside             | `profiler`                                               |

Two gitignored trees hold what the tree makes, each for one reader: `out/` is what the bakes and the asset ingest write, in the shape the bucket holds, and `cache/` is what `dev:oven` bakes on demand, deleted at no cost. What an ingest makes on its way into db-geo, the warped elevation, stays beside its script (`infra/geo/elevation/warped/`).

The casing in the tree is not mixed by accident. Rust, the xpute packages and the kernel's host (`apps/web/src/kernel/`) share one `snake_case` vocabulary across the boundary: `len`, `u32`, `new_len`. The rest of the shell and the scripts are `camelCase`, as TypeScript is. A name that crosses — a snapshot field, a packet's field, a catalog entry — keeps the kernel's.

---

## Learn More

[AGENTS.md](./AGENTS.md) is how to work in the tree, [doc/backlog.md](./doc/backlog.md) what is queued next, and [doc/minutes/](./doc/minutes/) why things went the way they did.

`xpute/` is xpute's repository root, developed here and copied out byte for byte by `xpute/script/export.ts` (`deno task export:xpute`): [xpute/README.md](./xpute/README.md) and [xpute/ARCHITECTURE.md](./xpute/ARCHITECTURE.md) are the system under the application — its memory, rings, turn, handles, contract and wire formats — with `xpute/LICENSE` and `xpute/THIRD_PARTY_NOTICES.md` beside them. Nothing under it names pixelet.

---

[github.com/canplane/pixelet](https://github.com/canplane/pixelet)
