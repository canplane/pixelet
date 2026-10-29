# pixelet

**[▶ Play](https://plei.me)**

> _A droplet fades. A pixelet remains._

_Started on December 15, 2025._

**pixelet puts the real Earth into cubes — your street, its buildings, the river
beside it, the hills behind it — and lets you build on it.**

<img alt="Voxel objects built on real-world terrain" src=".github/asset/screenshot.webp" width="100%">

The planet is too large to load or draw at once, so how finely anything is drawn
follows one rule: **the eye decides.** The view is a grid of cells of equal
angle about the eye, so a cell grows with distance. Space is cut into cubes at
every zoom; one cube of space at one zoom is a **node**, an address the world is
asked about, never something stored. Wherever it stands, the world is drawn at
the zoom whose nodes span a cell there (`pxl-spec-core grid.rs` `cell_at`). A
city, a mountain range or a tower behind a ridge costs the cells it covers on
the screen, not what it holds.

Every pixel's ray, every step a walker takes, the pointer and every edit ask the
world the same question: what fills this node? One function answers it, each
layer written over the one below (`pxl-spec-core world.rs` `node`). It is
compiled for the CPU and, through rust-gpu, for the GPU (`pxl-spec-core cast.rs`
`first`, `pxl-shader cast.rs` `cast_cs`), so what is drawn is what is walked on
and edited.

```text
  a pixel's ray      a walker's step      the pointer      an edit
   (the GPU)           (the CPU)          (the CPU)       (the CPU)
       └──────────────────┴──────────┬──────┴───────────────┘
                                     ▼
                         what fills this node?
                                     │
                  what players placed              ┐ each written
                  the ways off the ground          │ over the one
                  the ground's grid and its caves  ┘ below it
```

Real geography is baked ahead: [Overture Maps](https://overturemaps.org/)'
geometry, OpenStreetMap's coastline and
[GEDTM30](https://doi.org/10.5281/zenodo.18887460)'s elevation are painted into
tiles, fetched only when the view lacks one. What players build is kept as they
place it, and the trees and cars the ground grows are worked out where they
stand, never stored. The same world runs in a browser and on a device, through
one Rust kernel drawing with WebGPU.

---

## What runs where

The world lives in one Rust **kernel** (`pxl-kernel`, drawing through
`pxl-render`). It runs as the guest of [xpute](./xpute/README.md), which runs
one program inside another: the **host** owns the platform, grants the guest
one memory and a quota of time on each call into it, a **turn**, and does the
outside work the guest asked for once the turn returns.

The kernel's **bracket** plugs it into one of its forms. In a browser tab,
`pxl-wasm` makes it one WebAssembly module, which the page (`ts/app/web`) runs
on the browser's WebGPU; on a device, `pxl-native` links it into the app
(`ts/app/native`) as a library that runs its GPU calls on wgpu. Both apps bundle
the same **client** (`ts/client`), and each hands it, once, its own **client's
bracket**: the guest and what its platform gives, storage, text, the network,
the sensors and the surfaces (`ts/client/src/bracket.tsx`). The client is the
host (`src/host/`), the kernel's commands, snapshot and boot (`src/abi/`), what
answers its opens and writes (`src/namespace/`), and the **shell**
(`src/shell/`), the UI, which changes the world only by commands the kernel
applies.

Almost nothing is computed at request time. Tiles and assets are served from a
bucket under names that never come to mean something else, a tile under its
bake's version and an asset under a hash of its bytes; forward search alone
queries a database when asked.

| Layer     | Technology                                                                     |
| --------- | ------------------------------------------------------------------------------ |
| Shell     | TypeScript, React 19 in React Native: react-native-web and Vite on a page, Expo on a device |
| Kernel    | Rust, one WebAssembly module on a page, a static library in the device app     |
| Rendering | WebGPU (WGSL): the browser's, or wgpu over Metal or Vulkan on a device          |
| Edge      | Cloudflare Worker + R2                                                         |
| Servers   | Rust: axum, tokio-postgres, one per URL root, one database each, on one VM     |
| Databases | PostgreSQL: `db-main`, and `db-geo` with PostGIS                               |
| Geodata   | Overture Maps, OSM land polygons and GEDTM30, ingested into db-geo and baked   |

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

The four roots are written once, in `spec/spec/route.json`, and every relay
reads them there. A push to the `deploy` branch, once the checks and tests pass
here, rebuilds the servers, publishes the client and smoke-tests the result
([infra/README.md](./infra/README.md) has the procedures by hand).

---

## Getting Started

> Requires GNU Make, Python 3.11 or newer, [Deno](https://deno.land/) at the
> version `ts/.tool-versions` pins, Rust through [rustup](https://rustup.rs/)
> (the toolchain is `rust/rust-toolchain.toml`'s), Binaryen's `wasm-opt`, and
> Docker for the local databases and the shaders' build. `make check` and
> `make test` take what [xpute/README.md](./xpute/README.md#running-it) lists
> besides; publishing the bucket takes the AWS CLI, and a deploy by hand
> wrangler.

```bash
git clone git@github.com:canplane/pixelet.git
cd pixelet

cp ts/app/web/.env.example ts/app/web/.env
# for the databases and the servers, below:
cp infra/local/.env.example infra/local/.env
cp rust/server/.env.example rust/server/.env

make dev
```

The tiles come three ways, named as a bakery's bread is. `dev` has them
delivered: every server route is proxied to the live deployment, so no database
or server runs here. Bake work needs the oven, the whole stack baking each tile
when it is first asked for:

```bash
docker compose -f infra/local/docker-compose.yml up -d
make dev-oven
```

`dev-shelf` is that stack with the tiles taken from the last bakes under
`./out`, what a publish of them would show. `preview`, `preview-oven` and
`preview-shelf` are the same three over the production bundle, which is the one
to measure speed in.

| Target                          | Description                                                          |
| ------------------------------- | -------------------------------------------------------------------- |
| `make dev`                      | The client alone, every route from the live deployment               |
| `make dev-oven` / `dev-shelf`   | The client and its servers, tiles baked on demand / off the last bakes |
| `make preview[-oven\|-shelf]`   | The same over the production bundle                                  |
| `make dev-native`               | Metro, for the device app                                            |
| `make spec`                     | Generate Rust and TypeScript from the JSON under `spec/`             |
| `make serve-<geo\|api\|tile>`   | One server, or the dev tile server                                   |
| `make bake-<kind> [ARGS=…]`     | Make a kind's data: `terrain`, `place`, `division`, `earth`, `star`, `geoid` |
| `make ingest-<overture\|asset>` | Fill db-geo from Overture; hash `asset/content/` into `./out/asset`  |
| `make download-<what>`          | Fetch a source a tool reads: `pack`, `moon`, `earth`, `star`         |
| `make render-brand`             | The symbol's outline both apps bundle                                |
| `make deploy`                   | Build and publish the client with the Worker; CI runs it             |
| `make -j8 check` / `test`       | Every check / every test, or one tree's: `check-ts`, `test-rust`, …  |
| `make lint` / `fmt`             | Lint / format                                                        |

The repository's own scripts are run by their path: `script/publish.py` declares
which baked version of each kind clients read, `script/smoke.py` checks a
running deployment, `script/check-diff.py` checks the working tree's changes,
and `script/build-log.py` lists the published builds. What the bakes make is
under `./out`, emptied by hand after a re-ingest or a bake change (`rm -rf
out/oven`, or `out/<kind>*` to bake a kind from nothing); db-geo is filled by
each source's ingest ([infra/geo/README.md](./infra/geo/README.md)).

A development page puts two things in the browser console
(`ts/client/src/shell/debug/console.ts`): `xpute.cmd(command, ...args)`, which
sends the kernel any command by its number
(`spec/client/kernel/domain/control/cmd.json`), and `pxl.profiler`, the profiler
panel's `report()` and `trace()`.

The agent instructions are [AGENTS.md](./AGENTS.md) and the procedures
[.agents/skills/](./.agents/skills/), kept vendor-neutral; an assistant's own
directory links to them once per checkout:

```bash
mkdir -p .claude
ln -sfn ../AGENTS.md      .claude/CLAUDE.md
ln -sfn ../.agents/skills .claude/skills
```

### On a device

`ts/app/native` is the device's app: React Native through Expo, which bundles
the client, links the kernel in and takes every route from the live
deployment. Its native projects are generated by `expo prebuild` and not kept,
and the kernel is built with the app by a step of its native build
(`ts/app/native/script/build-native.py`). Setting a machine up, building and
installing are the [device-build skill](./.agents/skills/device-build/SKILL.md).

---

## Repository Structure

The tree is grouped by language, each a root of its own tool, and within a
language by where a thing runs.

```
.
├── ts/                  # The TypeScript workspace (Deno)
│   ├── client/          #   the client both apps bundle
│   │   └── src/
│   │       ├── host/    #     xpute's host, mounted
│   │       ├── abi/     #     the kernel's commands, snapshot and boot
│   │       ├── namespace/ #   what answers the kernel's opens and writes
│   │       └── shell/   #     the UI
│   ├── app/web/         #   the page's app and its bracket
│   ├── app/native/      #   the device's app, its native module and bracket
│   ├── server/edge/     #   the Cloudflare Worker that serves it all
│   └── build/           #   what the bundlers read the workspace through
├── rust/                # The Rust workspace (Cargo)
│   ├── spec/            #   pxl-spec: the model every crate shares
│   │   └── core/        #     pxl-spec-core: what CPU and GPU both compute
│   ├── client/          #   pxl-kernel, pxl-render, and pxl-shader, the GPU's
│   │                    #     compute entries built to WGSL
│   ├── client-bracket/  #   pxl-wasm and pxl-native, the kernel's two forms
│   ├── server/          #   pxl-geo (/geo), pxl-api (/api), pxl-server,
│   │                    #     pxl-geo-spec; pxl-geo-bake, the bakes
│   └── tool/            #   pxl-tool: what writes the files the tree keeps
├── spec/                # JSON every language's code is generated from
├── xpute/               # The system under it, its own repository's root
├── asset/               # The shipped models, texture pack and sky
├── script/              # The repository's own scripts (Python, sh)
├── infra/               # Compose files; db-geo's ingests
└── doc/                 # The record: backlog, minutes, experiments
```

What shares the model is Rust: the kernel, the servers and the bakes compile
`pxl-spec`. The client is TypeScript, and what it needs of the model, catalogs,
command numbers and routes, is generated into `ts/client/src/**/*.spec.ts` from
the same JSON.

Below the shell, Rust, xpute and the client's host, abi and namespace share one
`snake_case` vocabulary across the boundary; the shell and the scripts are
`camelCase`, as TypeScript is. A name that crosses, a snapshot field, a packet's
field or a catalog entry, keeps the kernel's.

---

## Learn More

[ARCHITECTURE.md](./ARCHITECTURE.md) follows the kernel part by part,
[AGENTS.md](./AGENTS.md) is how to work in the tree, and
[doc/backlog.md](./doc/backlog.md) what is queued next.

`xpute/` is xpute's repository root, developed here and copied out byte for
byte by `xpute/script/export.py`; [xpute/README.md](./xpute/README.md) and
[xpute/ARCHITECTURE.md](./xpute/ARCHITECTURE.md) describe the system under the
application. Nothing under it names pixelet.

---

[github.com/canplane/pixelet](https://github.com/canplane/pixelet)
