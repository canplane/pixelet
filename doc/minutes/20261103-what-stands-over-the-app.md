# What stands over the app

**2026-11-03** — recorded with the user. Main was at `430a3086b9`, as `original`
keeps it, which drew nothing: its client reads `division` in format 9 and the
published data is in 8, so the kernel refused the source's declaration
(`FS_MOUNT`, `EINVAL`), a warning in the console and `Loading area…` for good.

## What the user decided

- **A format this client does not read is a screen that says to update.** The
  session proposed switching off only the kind that differs, and a declaration
  with a version per format; the user took neither.
- **Broken midway is a panic, the blue screen; anything else is the white one.**
- **The white screen never comes over the app.** The app shows once everything
  the white screen waits on has passed.
- **The halts and panics are said in one place.**
- **The progress bar moves from the search strip to the white screen**, which
  holds until the world is first seen. No border, no rounding, no percent.
- **Everything still awaited goes to the progress screen**, what it waits on in a
  line under the bar: WebGPU, the network, the server, the shaders.
- **`Halt` was the wrong name.** It is now `Progress`, `Failed` and `Panic`
  (`shell/screens.tsx`), each with its locale keys.
- A login screen is to come; what is built leaves room for it.

## What was built

`coverOf` (`shell/cover.ts`) is the one place, and `shell/cover.test.ts` its
table. `Boot` gained a stage (`abi/state.ts`): `BOOT_STARTING`, the kernel, the
first GPU answer and the declaration; `BOOT_LOADING`, the app mounted under the
cover, as pipelines are made only with the world's canvas (`render_frame`);
`BOOT_STARTED`, once the view is drawn or past the service area. `FS_MOUNT` is now
a call its refusal comes back from. `GPU_ASKING` is the first ask, apart from
`GPU_WAITING`, a retry.

Every screen comes from `coverOf`, on the page as on a device: the page boots as the
device does, a browser with no WebGPU or a page that is no secure context is a `Gpu`
(`GPU_UNSUPPORTED`, `GPU_INSECURE`), and the first ask for an adapter is retried as a
later one is. `useStanding` (`shell/boot.ts`) gathers what it reads, and `Screen`
(`shell/screens.tsx`) draws what it says. A throw while the shell renders is a panic
too (`ErrorBoundary`, `SHELL_PANIC`).

On the page the ground comes first: `index.html` holds a `div` of it in `#root`
(`%GROUND%`, `shell/ui/ground.ts`) until the shell mounts over it, so nothing black
shows before the white screen. `#root` and `body` stay black under the world.

A login screen would be another kind of `Cover`, over `BOOT_STARTING` or
`BOOT_LOADING`, so the world is made while it stands.

## Against a router

The user asked whether TanStack Router would do. The session recommended against
it: this is the boot's state and no URL, and the canvas has to stay mounted under
the cover. Not decided further.

## Const enums

The user asked why they remain. They were inlined so the published bundle gives
none of their names away
([what the published build gives away](./archive/20260918-what-the-published-build-gives-away.md)),
13 of them, kept by `ts/build/const-enum.ts` for Vite and the device's Babel, at
the cost of restarting the dev server when one changes.

**The user decided they go, and by what a value is.** What is the ABI, the numbers
in `abi/state.ts`, is constants under one prefix in the one file, as a uapi header
has them (`BOOT_STARTING`). What is the shell's and read across its files is
`export const` in a file of its own and `import * as`, as `abi/cmd/*.spec.ts` is
(`DockMode`, `ToastTone`, `BackingPlace`). What one file reads is a plain `const`.
No object: a bundle would carry its names. `ts/build/const-enum.ts` and both
bundlers' passes are gone, and `erasableSyntaxOnly` in `ts/tsconfig.json` refuses
an enum.

The web bundle carries none of the names. Metro inlines nothing across files, so
the device's holds the exported ones, as it holds `abi/cmd/*.spec.ts`'s. **The
user decided a pass for release builds alone will inline them, later**: it reads
`export const` of a literal across an import, for `abi/cmd/*.spec.ts` too, and a
hash of those files in Metro's `cacheVersion` keeps a changed value from coming out
of the cache.

> Since 2026-11-10 (7d6a786b0f): built as `inline` in `ts/client/babel.config.ts`,
> release builds only. It writes a literal where an importer reads it and where
> its own module reads it, and tree shaking drops an export no read names. A
> binding's handle and method numbers keep their names where a module imports the
> binding whole (`import * as`), as the bundle keeps every export of such a
> module.

## The location and the long wait

**The user decided**, on what the session proposed:

- **A location prompt no one answers holds the world 5 s.** The line under the bar
  says so (`progress.location`); then the driver reports a timeout
  (`driver/sensor.ts`, only while the prompt stands), the kernel opens at its
  fallback, and no toast calls it a failure. A later answer moves the view.
- **The wait for the area gets a share of the bar**: half the renderer's, half the tiles
  asked for that show, from the kernel (`area_progress`). It may go back as more are
  asked for; nothing holds it up, the user having said no ad hoc is wanted for it.

## Open

- On iOS the location prompt is the system's: the app is inactive under it, so the
  5 s holds nothing open and the prompt must be answered.

## A stop on a device

A kernel that stopped on a device took the process with it, so no panic screen
showed there. **The contract changed**: stop tells a form last (`Stop::End`), a native
host ends the guest's thread there, parked as it stands, and the process goes on;
`xpute_run` answers NaN, which the app's guest throws as a module traps. The doorbell
throws a trap at no one: it tells `stopped`, and a call answers `-ENOTRECOVERABLE`.
Forced on the iOS simulator and on the page, both show the blue screen with the stop
code. No phone has run this.

## On a device

The native app had not run since `ActiveTool`'s member `Object` (`var Object` under
Metro, hiding the global: now `Cube`, and the generator refuses the name) and
`atmosphere.rs`'s read back (`landed_words` off the guest's memory on a native build:
now on the heap, as `tally.rs` keeps its own). A fresh install also met `Intl.Locale`,
absent from the iOS simulator's Hermes, as a saved language had kept it unread. On the
iOS simulator, against local tiles, the app now goes from the progress screen to the
world. No phone has run this.
