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
| `deno task lint`      | Lint                         |
| `deno task fmt`       | Format                       |

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

## Learn More

Curious how the coordinate system, terrain bake pipeline, or wire format work under the hood? See [ARCHITECTURE.md](./ARCHITECTURE.md).
