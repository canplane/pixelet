# One toolchain, because every setting broke another

**2026-10-28** — recorded with the user, reconstructed from history. Written
against main at `c9e157b79`. It records why the TypeScript side left pnpm + Node
for a Deno workspace in January 2026, a move whose commit (`361c796e6`, "full
platform rewrite — xpute runtime, pxl engine, Deno migration") lists what it
deleted but never says why. That commit is a squash: its author date is
2026-01-19 and its commit date 2026-04-03, so nothing between `d202ce988`
(01-07) and April survives, and February has no history at all. What follows is
read from the commits before it, their configs and comments, and the user's
memory of them; where a message names a cause the diff does not bear out, it is
marked.

> It was not that anything was heavy. Changing one setting made the build or the
> lint fall over, demanding package files that were not there, and the shared
> package kept leaving build output beside its sources. Out of that came the
> habit of keeping configs as symmetric as possible and the tools as few as
> possible: give the server and the client the same tools — bundle the server
> with Vite too.

## The user's account

- Changing one setting made the tools complain that package files were missing.
  NestJS itself was not much of an issue. The trouble was not weight: the build
  or the lint itself broke, with files like `.js.map` as an example.
- The shared package kept leaving a build cache as siblings of its sources,
  `.js` files made over and over, and it had to be ignored in `.gitignore` or
  the VS Code settings.
- After that pain, as the user recalls it, the idea grew of keeping configs as
  symmetric as possible and reducing the tools to a few: let the server and the
  client have the same tools, for instance bundle the server with Vite too.

## What the history shows

The tree then: `apps/web` (Vite, browser), `apps/api` (NestJS on Node from
12-26), and a shared package (`@pixelet/shared`, renamed `@pixelet/core` on
01-04) both imported as TypeScript source.

| commit · date                        | what changed                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `73a23f885` · 12-17                  | pnpm workspace, Changesets, `.nvmrc`, Prettier, ESLint. The shared package exports `./src/index.ts` directly.                                                                                                                                                                                                                                                                                   |
| `9641a9cb1` · 12-19                  | `composite`/`incremental` and `tsc -b` for web and api, "for performance" (never measured).                                                                                                                                                                                                                                                                                                     |
| `c1d3ab292` · 12-26                  | Project References. The shared tsconfig is `composite` + `declaration` + `declarationMap` + `rootDir: "src"` with **no `outDir`**. The same commit adds "TypeScript build outputs (Project References)" to `.gitignore` (`**/src/**/*.js`, `*.js.map`, `*.d.ts` …) and hides `**/src/**/*.js` in `.vscode` `files.exclude`. The api tsconfig moves to tsup "for symmetry with the web project". |
| `be5fe874f` · 12-26                  | NestJS: `nest-cli.json` with `builder: webpack`, `verbatimModuleSyntax: false` for its decorators, `noEmit: false`. VS Code stops offering `.d.ts` files as auto-imports.                                                                                                                                                                                                                       |
| `b78d115ed` · 12-27                  | api: `composite: false` "to lift the constraint", `rootDir: "../.."` "the key fix", references off in dev; webpack `nodeExternals` allowlist for the shared package. The shared package's `"."` export is removed.                                                                                                                                                                              |
| `2a5213c04` → `b5e070bca` · 12-28/29 | a web file is included into the shared and api tsconfigs "for better type resolution", then reverted the next day.                                                                                                                                                                                                                                                                              |
| `dc9c8b8a5` · 01-04                  | tsconfigs split again (`tsconfig.app.json`, `tsconfig.lib.json`), `composite` back on; `init.sh` wipes `node_modules`, `dist` and `*.tsbuildinfo`.                                                                                                                                                                                                                                              |
| `ea6db1fc4` · 01-04                  | the shared package gets **`outDir: "dist"`**, and the `.gitignore` and VS Code hiding rules are commented out. `.npmrc` gains `public-hoist-pattern` for tfjs and i18next. Its title is about i18n.                                                                                                                                                                                             |
| `72e6e0f89` · 01-04                  | the shared → core rename touches the alias in six places at once: root tsconfig paths, the Vite alias, the web and api tsconfig includes, the webpack allowlist, ESLint.                                                                                                                                                                                                                        |
| `361c796e6` · 01-19                  | pnpm, every tsconfig, ESLint, Prettier, `.npmrc`, `.nvmrc`, `.changeset/`, nest-cli and webpack go. A root `deno.json` and one per member come in, and `apps/api` gains a `vite.config.ts`.                                                                                                                                                                                                     |

## Why it hurt

- **Build output beside the sources.** A `composite` project must emit
  declarations, and with `rootDir: "src"` and no `outDir`, `tsc -b` writes
  `.js`, `.d.ts` and their maps into `src/` itself. That is the sibling cache
  the user remembers, and `c1d3ab292` hid it rather than stopped it. `ea6db1fc4`
  stopped it with `outDir`, under an i18n title.
- **Settings coupled across files.** `rootDir`, `composite` and the references
  flipped three times in nine days (12-19, 12-26, 12-27, 01-04); the shared
  package's `exports`/`main`/`types`/`publishConfig` changed in four commits;
  the path alias lived in six files. A change in one tsconfig broke the
  resolution another relied on, which is the "package files are missing" the
  user remembers. The only error code written down is TS6059 (a file outside
  `rootDir`), in a `c1d3ab292` comment; the web tsconfig at `d202ce988` still
  pairs `rootDir: "src"` with including the core's sources, the shape that
  raises it.
- **A second bundler and a second resolver.** The day `c1d3ab292` moved the api
  to tsup "for symmetry with the web project", NestJS brought webpack, a
  decorator exception to `verbatimModuleSyntax`, and an allowlist to bundle the
  shared sources. Nest's weight is never complained of anywhere; what it brought
  was asymmetry.

## What replaced it

- One workspace in the root `deno.json`; each member a `deno.json` that exports
  its sources (`"./*": "./src/*"`). Nothing is emitted, so nothing can land
  beside a source, and no ignore rule is needed for it.
- The alias in one place: the root `imports`
  (`"@pxl/spec/": "./packages/pxl/spec/src/"`), replacing tsconfig paths, the
  Vite alias and the webpack allowlist.
- Eight tsconfigs became the root `compilerOptions` plus a member's `types`;
  ESLint and Prettier became `deno lint` and `deno fmt`; `tsc -b` became
  `deno check`; `.npmrc` hoisting became `nodeModulesDir: "auto"`.
- The server is bundled the way the page is. The api became Hono, built by
  Vite's SSR build. The README of that commit: "The monorepo, package
  management, and task execution are unified under Deno. Both the web and api
  apps use Vite as the bundling/build entry."

No sibling output and no TS6xxx-style breakage shows up in the history after it.
`package.json` came back only where Expo and Metro need one (`99510ee24`,
09-26), still run through Deno.

## The principle it left

Configs symmetric across server and client, and the tools few: one runtime, one
bundler, one formatter, one linter, each at its defaults. The symmetry was
already wanted in December (the `c1d3ab292` comment) and broken the next day by
a second bundler. Later commits keep to it: the device bundle writes constants
"as Vite's define and const_enum write them into the web's" (`0a1397d2d`),
"every formatter at its own defaults" (`1a6281ee7`), the editor settings hold
"only what the editor alone is told" (`6356b27e5`). No rule in `AGENTS.md` or a
skill states it; this record is the first to. It is not the same as making both
sides one artifact:
[`20260918-the-server-in-rust-and-the-line-already-drawn.md`](./20260918-the-server-in-rust-and-the-line-already-drawn.md)
rejects "both sides being WASM" because the symmetry is already had at the
source.

## Doubts

- `361c796e6` says nothing about why; the reasons above are the user's memory
  checked against the commits before it.
- Some messages misname their cause: `be5fe874f` fixes `.env` loading "in an Nx
  monorepo" with no Nx in the tree; `ea6db1fc4`'s i18n title hides the `outDir`
  fix; `9641a9cb1`'s "for performance" was never measured; `2a5213c04`'s "better
  type resolution" was a symptom, reverted a day later.
- That the lint tripped on the generated files is the user's memory with no
  message or comment to confirm it. The lint and format globs did cover `.js`
  under `src/`, and ESLint ignored only `node_modules` and `dist`.
- [`archive/20260708-vision-and-scope-pivot.md`](./archive/20260708-vision-and-scope-pivot.md)
  says tools like Deno were "partly being exercised for their own sake", so part
  of the move may have been the trying of it.
