# pixelet

> _A droplet fades. A pixelet remains._

_Started on December 15, 2025._

---

**pxl** (pixelet) is a personal spatial engine project, with **xpute** (Transpute) emerging from the same codebase as a shared distributed runtime.

---

## xpute (Transpute)

A runtime that treats browser workers and server runtimes as first-class peers under the same IPC model. Browser and server are symmetric by design.

### IPC

Two frame planes share the same link:

| Plane  | Frame Kind | Addressed by     |
| ------ | ---------- | ---------------- |
| Signal | `SIGNAL`   | Vector (u16)     |
| Post   | `POST`     | Command (string) |

Both planes support `ACKREQ` (request/response) and `UNACK` (fire-and-forget) semantics. The `RES` flag is owned by the router — devices and dispatchers never set it directly.

### Workers

Workers are numbered. Two IDs are reserved:

| ID    | Role                    |
| ----- | ----------------------- |
| `1`   | Main thread             |
| `255` | Guest (network gateway) |

Workers register their vector/command capabilities during boot via a `LINK_INIT_SYN` / `LINK_INIT_ACK` handshake. The `Switch` maintains the routing table and forwards frames accordingly.

### Dispatch

| Component      | Role                                                   |
| -------------- | ------------------------------------------------------ |
| `Switch`       | Frame router. Maintains vector/command → worker table. |
| `VectorPort`   | Handles vector-addressed dispatch.                     |
| `CommandPort`  | Handles command-addressed dispatch.                    |
| worker runtime | Owns ACKREQ/UNACK semantics at the runtime boundary.   |

### Network

`NetGateway` manages the network boundary:

- HELLO handshake with HMAC cookie (anti-amplification)
- Per-packet AEAD encryption (X25519 key exchange)
- Ed25519 packet signatures
- Replay protection with per-peer RID cache

Two transports are supported:

| Transport          | Kind                               |
| ------------------ | ---------------------------------- |
| `CentralTransport` | WebSocket + HTTP (server-mediated) |
| `MeshTransport`    | WebRTC (peer-to-peer)              |

### XTP — Xpute Tree Packet

Binary wire format for IPC/RPC body payloads.

- **Tree-shaped**: scalar leaves, typed-array sequences, branch nodes with positional children
- **C-layout-compatible**: 8-byte word alignment, little-endian, relative offsets throughout
- **Relocatable**: all internal references are relative; packets can be moved or spliced freely
- **Zero-copy reads**: typed array sequences are returned as views into the packet buffer
- **GRAFT**: a pre-encoded subtree packet splices inline with identical on-wire layout to an inline-encoded node
- **Type-preserving nulls**: optional nodes preserve type metadata even when physically absent

TLV is used separately for IPC frame argument encoding (flat, self-delimiting, stream-oriented).

---

## pxl (pixelet)

A spatial map engine built on xpute, designed around voxel-style 3D representation of geographic data.

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

Pages are decoded into render-ready terrain data for the terrain layer. LOD is distance-based with reference distances at L1 (512), L2 (1024), L4 (2048), L8 (4096), L16 (8192), and L32 (∞).

---

## Repository Structure

```
.
├── apps/
│   ├── web/                        # Web app (Deno + Vite)
│   └── api/                        # API server (Deno + Hono + Vite)
│
└── packages/
    ├── xpute/
    │   ├── core/                   # Switch, worker runtime, NetGateway, transports
    │   └── spec/                   # IPC frames, XTP, TLV, RPC, crypto, error types
    │
    └── pxl/
        ├── engine/                 # Core spatial engine runtime
        ├── bake/                   # OSM processing and page baking pipeline
        ├── render/                 # LOD system, chunk render data types
        ├── spec/                   # Cell types, PVF3, spatial indexer, adapters
        └── common/                 # Shared utilities
```

---

## Tech Stack

The monorepo, package management, and task execution are unified under Deno.
Both the web and api apps use Vite as the bundling/build entry.

| Layer               | Technology                                   |
| ------------------- | -------------------------------------------- |
| Runtime / Workspace | Deno + TypeScript (strict)                   |
| Bundler             | Vite                                         |
| Web UI              | React 19                                     |
| API                 | Hono                                         |
| 3D Rendering        | Three.js + React Three Fiber                 |
| Cryptography        | @noble/curves, @noble/ed25519, @noble/hashes |
| Networking          | WebSocket (central), WebRTC (mesh)           |
| Geodata             | OpenStreetMap (OSM)                          |
| Spatial math        | gl-matrix, PVF3 fixed-point                  |

---

## Getting Started

> Requires [Deno](https://deno.land/) 2.x.

```bash
git clone git@github.com:canplane/pixelet.git
cd pixelet

cp .env.example .env
cp apps/web/.env.example apps/web/.env

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
