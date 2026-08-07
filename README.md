# pixelet

> _A droplet fades. A pixelet remains._

_Started on December 15, 2025._

---

**pxl** (pixelet) is a location-based social/building game — real-time presence, voxel-style building, and a persistent social space — built around a voxel-style 3D representation of real-world (OSM) geographic data.

The engine is split into a pixelet-specific domain layer (**`@pxl/shared`**: coordinate system, cell/terrain model, bake wire format) and a project-agnostic utility layer (**`@xpute/core`**: generic collections, codecs, and the binary tree protocol those formats are built on) — see [Repository Structure](#repository-structure).

> An earlier iteration of this codebase explored a decentralized P2P/WebRTC runtime (`xpute` as a peer-symmetric IPC substrate: workers, `Switch` frame routing, `NetGateway` transports), then a Rust/WebGPU rewrite of the spatial/bake stack. Both directions are parked in favor of a plain client-server MVP. The old code isn't deleted — it's preserved on `checkpoint/20260107-float`, `checkpoint/20260405-xpute`, and `checkpoint/20260409-wasm-aborted`.

---

## pxl (pixelet)

A spatial map engine designed around voxel-style 3D representation of geographic data.

### Coordinate System — PVF3

PVF3 (Packed Vector Fixed-point 3D) is a 96-bit fixed-point coordinate type:

```
PVF3 = { i: IVec (64-bit), f: FVec (32-bit) }
```

- **IVec**: three signed integer lanes in 24-24-16 asymmetric layout (X, Y, Z). The X axis is cyclic — it wraps to handle longitude seam.
- **FVec**: three unsigned fractional lanes at 10-bit precision (1/1024 resolution per axis).

All arithmetic is deterministic across platforms via BigInt. Physical meaning (meters, tiles, voxels) is assigned by the caller through scaling.

### Spatial Hierarchy

Five levels of spatial partitioning, each a power-of-two subdivision of the level above:

| Level | Name   | Unit                        |
| ----- | ------ | --------------------------- |
| L1    | Block  | Finest grain                |
| L2    | Chunk  | 2^6 blocks                  |
| L3    | Page   | 2^8 blocks (streaming unit) |
| L4    | Anchor | 2^12 blocks                 |
| L5    | Shard  | Coarsest grain              |

Pages (L3) are the primary streaming unit. The page scheduler requests them by Chebyshev distance from the current center, in a demand-paged style.

### Ground Channels

Each page encodes four ground channels rasterized from OSM geometry:

| Channel     | Contents                                            |
| ----------- | --------------------------------------------------- |
| `LAND`      | Ground surface type (bare, green, sand, rock, etc.) |
| `WATER`     | Inland water body type (river, lake, wetland, etc.) |
| `STRUCTURE` | Buildings and constructed surfaces                  |
| `TRANSPORT` | Roads, paths, railways                              |

Sea cells carry a flag bit (`SEA_FLAG = 0x80`) to distinguish sea from land using the same channel byte.

### Bake Pipeline

OSM raw data → rendered page:

```
raw XTP packet
  → TreeReader (XTP decode)
  → osm_adapter (row → feature)
  → osm_normalizer (classify, rank, assign tags)
  → ground_baker (rasterize geometry into ground channels)
  → encode_terrain (elevation + ground → XTP)
```

Polygon rasterization uses even-odd fill with hole support. Linear features use a thickness kernel. Arena-backed scratch allocation is used for geometry work to reduce GC pressure.

### Rendering

Pages are decoded into render-ready terrain data for the terrain layer, rendered with Three.js / React Three Fiber. LOD is distance-based, stepping cell density by 1/2/4/8/16/32 at reference distances of 96, 192, 384, 768, 1536, and ∞ world units.

### XTP — Xpute Tree Packet

The binary wire format the bake pipeline's pages (and the map-tile API response) are encoded as.

- **Tree-shaped**: scalar leaves, typed-array sequences, branch nodes with positional children
- **C-layout-compatible**: 8-byte word alignment, little-endian, relative offsets throughout
- **Relocatable**: all internal references are relative; packets can be moved or spliced freely
- **Zero-copy reads**: typed array sequences are returned as views into the packet buffer
- **GRAFT**: a pre-encoded subtree packet splices inline with identical on-wire layout to an inline-encoded node
- **Type-preserving nulls**: optional nodes preserve type metadata even when physically absent

TLV is used separately for flat, self-delimiting, stream-oriented argument encoding.

Lives in `@xpute/core` (not `@pxl/shared`) — it's a generic relocatable-binary-tree codec with no pixelet domain knowledge baked in.

---

## Repository Structure

```
.
├── apps/
│   ├── web/                        # Web client (Deno + Vite + React 19 + R3F)
│   └── api/                        # API server (Deno + Hono), OSM bake pipeline
│
└── packages/
    ├── xpute/
    │   └── core/                   # Generic kit/collections/io/errors + XTP binary protocol
    │                                # (no pixelet domain knowledge — portable to other projects)
    │
    └── pxl/
        └── shared/                 # PVF3 coordinate system, cell/terrain model, bake wire format
                                     # (pixelet-specific — must stay identical between web and api)
```

---

## Tech Stack

The monorepo, package management, and task execution are unified under Deno.
The web app uses Vite as its bundling/build entry; the api runs directly under Deno.

| Layer               | Technology                      |
| ------------------- | ------------------------------- |
| Runtime / Workspace | Deno + TypeScript (strict)      |
| Bundler (web)       | Vite                            |
| Web UI              | React 19                        |
| API                 | Hono                            |
| 3D Rendering        | Three.js + React Three Fiber    |
| Database            | PostgreSQL (main + geo/PostGIS) |
| Geodata             | OpenStreetMap (OSM)             |
| Spatial math        | PVF3 fixed-point (BigInt)       |

---

## Getting Started

> Requires [Deno](https://deno.land/) 2.x and Docker (for the local Postgres databases).

```bash
git clone git@github.com:canplane/pixelet.git
cd pixelet

cp .env.example .env
cp apps/web/.env.example apps/web/.env

docker compose --env-file .env -f infra/docker-compose.yml up -d

deno task dev
```

| Task                  | Description                  |
| --------------------- | ---------------------------- |
| `deno task dev`       | Start web + api concurrently |
| `deno task dev:web`   | Frontend only                |
| `deno task dev:api`   | Backend only                 |
| `deno task build:web` | Build frontend               |
| `deno task build:api` | Build backend                |
| `deno task check`     | Type-check all packages      |
| `deno task lint`      | Lint                         |
| `deno task fmt`       | Format                       |
