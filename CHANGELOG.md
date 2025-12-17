# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

- No changes yet.

---

## [0.1.0] - 2025-12-17

### Added

- **Monorepo & Workspace**:
  - Initialized pnpm-based monorepo with `apps` (web, api) and `packages` (shared).
  - Configured pnpm workspaces and Changesets for versioning.
- **Frontend Architecture (FSD)**:
  - Implemented **Feature-Sliced Design (FSD)** methodology.
  - Organized layers into `app`, `pages`, `features`, and `shared`.
  - Established naming conventions: **kebab-case** for files and **Named Exports** for core logic.
- **Core Tech Stack**:
  - **3D & Graphics**: Three.js integration with React Three Fiber (R3F) and Drei.
  - **Styling**: Tailwind CSS with CVA (Class Variance Authority) for a scalable UI system.
  - **UI/UX**: Lucide React icons, Lottie animations, and SVGR/SVGO optimization.
  - **Infrastructure**: react-i18next for i18n, date-fns for date utilities, and TanStack Query + Zustand for state management.
- **Global UI System**:
  - Centralized `ModalManager` and imperative `UiController` with Proxy-based safety.
  - Global `DevPanel` and `/dev` routes for internal debugging and experiments.
- **Backend & Tooling**:
  - API scaffold using Node.js and Fastify.
  - Docker-based local PostgreSQL setup.
  - Performance-first ESLint (flat config) and Prettier baseline.
  - Comprehensive documentation and development workflow in README.

### Changed

- N/A (Initial release including architecture baseline)
