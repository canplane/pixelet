# pixelet

> _A droplet fades._
> _A pixelet remains._

_Started on December 15, 2025._

---

Pixel-based map & UX experiments.

---

## Overview

pixelet is a **pnpm-based monorepo** project consisting of:

- **Web**: React + Vite frontend (FSD Architecture)
- **API**: Node.js backend
- **Shared**: Shared TypeScript types and utilities

The goal is to provide a fast, flexible development environment with clear boundaries between client, server, and shared code.

---

## Tech Stack

### Frontend (Modern Stack)

- **Framework**: React 19 + Vite
- **Architecture**: **Feature-Sliced Design (FSD)**
- **Styling**: Tailwind CSS + CVA (Class Variance Authority)
- **3D Engine**: Three.js + React Three Fiber (R3F) + Drei
- **State Management**: TanStack Query (Server), Zustand (Client)
- **Utilities**:
  - `lucide-react` (Icons)
  - `date-fns` (Date handling)
  - `react-i18next` (Internationalization)
  - `Lottie` (Animations)
- **Assets**: SVGR & SVGO for optimized vector handling

### Backend

- **Framework**: Node.js + Fastify
- **Database**: PostgreSQL (Docker-based)

### Tooling

- **Package Manager**: pnpm (Workspaces)
- **Versioning**: Changesets
- **Quality Control**: ESLint (Performance-first flat config), Prettier
- **Environment**: Docker / Docker Compose

---

## Repository Structure

```
.
├── apps/
│   ├── web/                # Frontend (Vite + React)
│   │   └── src/
│   │       ├── app/        # System init, Global providers, Styles, Router
│   │       ├── pages/      # Page-level components (Default exports)
│   │       ├── features/   # Domain-specific business logic (e.g., map, auth)
│   │       └── shared/     # Reusable UI (button, input), Libs (i18n, utils)
│   └── api/                # Backend (Fastify)
├── packages/
│   └── shared/             # Shared logic across monorepo
├── docker-compose.yml      # PostgreSQL setup
└── pnpm-workspace.yaml
```

---

## Development Standards & Conventions

### Naming Conventions

- **Files**: `kebab-case.tsx` (e.g., `app-router.tsx`, `modal-manager.tsx`)
- **Components**: `PascalCase` (e.g., `export function MapViewer`)
- **Hooks**: `camelCase` (e.g., `useUiController`)

### Module Rules

- **Named Exports Only**: Used for all system/logic components to ensure IDE auto-completion and refactoring stability (excluding `pages/` for lazy loading).
- **Strict Boundaries**: Layers should only import from lower layers (e.g., `features` can import from `shared`, but not from `app`).
- **Imperative UI Control**: Global UI states (Modals, Overlays) are managed via a centralized `UiController` with Proxy-based safety.

---

## Requirements

- **Node.js >= 20**
- **pnpm** (via Corepack recommended)
- **Docker** (for PostgreSQL)

### Enable pnpm (recommended)

```bash
corepack enable
corepack prepare pnpm@latest --activate
```

Verify:

```bash
pnpm -v
```

---

## Getting Started

> ⚠️ Always run commands from the **repository root**.

### 1. Clone the repository

```
git clone git@github.com:canplane/pixelet.git
cd pixelet
```

---

### 2. Install dependencies

```bash
pnpm install
```

---

### 3. Set up environment variables

Each app manages its own environment variables.

```bash
# API server, Infrastructure
cp .env.example .env

# Web app
cp apps/web/.env.example apps/web/.env
```

> Edit each `.env` file if you need custom ports, credentials, or API keys.
>
> ⚠️ Never commit `.env` files. Only `.env.example` files should be tracked in Git.

Each app manages its own environment variables.

---

### 4. Start PostgreSQL (Docker)

```bash
docker compose up -d
```

---

### 5. Run development servers

```bash
pnpm dev
```

This will start:

- Web app: [http://localhost:5173](http://localhost:5173)
- API server: [http://localhost:3000](http://localhost:3000)

---

## Scripts

### Development

```bash
pnpm dev        # Run web + api concurrently
pnpm dev:web    # Run frontend only
pnpm dev:api    # Run backend only
```

### Build

```bash
pnpm build       # Build web + api
pnpm build:web   # Build frontend
pnpm build:api   # Build backend
```

### Code Quality

```bash
pnpm lint
pnpm lint:fix
pnpm format
pnpm format:check
pnpm typecheck
```

### Versioning

```bash
pnpm changeset   # Generate a new versioning changeset
```

---

## Development Notes

- **Server state** is handled by **TanStack Query**
- **Client/UI state** is handled by **Zustand** or local React state
- Avoid deep imports from `@pixelet/shared/src/*`
- Shared package APIs must be accessed via declared exports only

---

## Linting & Formatting

- ESLint is configured with a performance-first setup
- Unused variables are reported as **warnings**
- Formatting is enforced via Prettier

---

## Contributing

1. Create a feature branch
2. Keep commits small and descriptive
3. Run `pnpm lint` and `pnpm typecheck` before pushing

---

## Philosophy

pixelet is built on the idea that maps and interfaces do not have to be literal representations of reality.

Instead of focusing on accuracy first, pixelet explores abstraction, reduction, and reinterpretation — treating pixels as expressive units rather than raw data points.

The goal is not to show _everything_, but to show _just enough_.

---

## History & Milestones

- **December 15, 2025** — Project started
- Initial pnpm-based monorepo setup (web, api, shared)
- Development tooling and linting baseline established

---

## License

Private project.
All rights reserved.
