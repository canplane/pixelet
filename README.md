# pixelet

> _A droplet fades. A pixelet remains._

_Started on December 15, 2025._

**pxl** (pixelet) is a location-based social/building game — real-time presence, voxel-style building, and a persistent social space — built around a voxel-style 3D representation of real-world (OpenStreetMap) geographic data.

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
| `deno task test`      | Run all tests                |
| `deno task lint`      | Lint                         |
| `deno task fmt`       | Format                       |

---

## Tech Stack

The monorepo, package management, and task execution are unified under Deno.
The web app uses Vite as its bundling/build entry; the api runs directly under Deno.

| Layer               | Technology                                   |
| ------------------- | -------------------------------------------- |
| Runtime / Workspace | Deno + TypeScript (strict)                   |
| Bundler (web)       | Vite                                         |
| Web UI              | React 19                                     |
| API                 | Hono                                         |
| 3D Rendering        | Three.js + React Three Fiber                 |
| Database            | PostgreSQL (main + geographic/PostGIS)       |
| Geodata             | OpenStreetMap (OSM)                          |
| Spatial math        | PVF3 packed fixed-point (BigInt)             |
| Terrain transport   | XTP typed-array packets over compressed HTTP |

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

The engine is split into a pixelet-specific domain layer (`@pxl/shared`) and a project-agnostic utility layer (`@xpute/core`) that `@pxl/shared`'s binary formats are built on.

---

## World and Rendering Model

Storage, network transfer, and rendering deliberately use different scales. See [ARCHITECTURE.md](./ARCHITECTURE.md#spatial-hierarchy) for the full L0–L5 hierarchy and the layer/space/shape terminology it's built from — in short:

- **L1 Block** (`1u` per axis): the canonical cell — a **Voxel** once it carries a material payload.
- **L2 Chunk** (`64` blocks per axis): the renderer's render/LOD/raycasting unit. The renderer composes it into a `64³` **Volume** per occupied **Band** (one signed-Z slice), grouping all occupied L2 bands at one XY address into one column component.
- **L3 Page** (`1024` blocks per axis): terrain's streaming/storage unit — terrain stores a `1024²` **Tile** there, X/Y only.
- **L4 Anchor** (`4096` blocks per axis): the floating-origin frame. `get_local_world_origin` snaps the camera's map center down to its L4 base; `AnchorRegistry` keeps one ref-counted `THREE.Group` per live L4 index and re-bases every group's local position when that snapped origin shifts. Carries no Tile/Volume payload of its own — a pure addressing/reparenting mechanism, proof that a space level doesn't need a shape at all.
- **L5 Shard** (`65536` blocks per axis): reserved for a future global sharding domain — not implemented (`L5_SHAMT` stays commented out in `indexer.ts`). Once active, its Z lane collapses (Z is only 16 bits, narrower than `L5_SHAMT`), so L5 can only ever address X/Y.

Terrain (a Tile-shaped layer) stays dense 2.5D page data; placed objects (a Volume-shaped layer) stay sparse octrees in source storage. Object leaf byte `0` is `UNDEF` (sparse absence/fallback); every explicit object-layer value owns the high bit, including explicit AIR used for terrain removal. At render time, both implement the same `WorldLayer` contract and paint into one temporary signed-Z Volume of Voxels. The same composed boxes drive meshing and raycasting, so rendered and clickable geometry cannot diverge by using separate approximations.

Dense `64³` Volumes exist only while composing and meshing. Long-lived state keeps compact surface data and GPU-ready instance buffers instead, avoiding the mobile memory cost of retaining a dense Volume for every visible chunk. Most of this pipeline is renderer-independent typed-array math; Three.js / React Three Fiber owns the final scene, material, texture, frustum, and GPU submission boundary.

---

## Learn More

Curious how the coordinate system, spatial hierarchy, unified world-layer pipeline, or wire format works under the hood? See [ARCHITECTURE.md](./ARCHITECTURE.md).
