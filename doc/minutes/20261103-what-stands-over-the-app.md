# What stands over the app

**2026-11-03** — recorded with the user. Main was at `8df835046`, which drew
nothing: its client reads `division` in format 9 and the published data is in 8,
so the kernel refused the source's declaration (`FS_MOUNT`, `EINVAL`), a warning
in the console and `Loading area…` for good.

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
table. `Boot` gained a stage (`abi/state.ts`): `Starting`, the kernel, the first
GPU answer and the declaration; `Loading`, the app mounted under the cover, as
pipelines are made only with the world's canvas (`render_frame`); `Started`, once
the view is drawn or past the service area. `FS_MOUNT` is now a call its refusal
comes back from. `Gpu.Asking` is the first ask, apart from `Waiting`, a retry.

A login screen would be another kind of `Cover`, over `Starting` or `Loading`, so
the world is made while it stands.

## Against a router

The user asked whether TanStack Router would do. The session recommended against
it: this is the boot's state and no URL, and the canvas has to stay mounted under
the cover. Not decided further.

## Const enums

The user asked why they remain. They were inlined so the published bundle gives
none of their names away
([what the published build gives away](./archive/20260918-what-the-published-build-gives-away.md));
12 remain, kept by `ts/build/const-enum.ts` for Vite and the device's Babel, at the
cost of restarting the dev server when one changes. Open.

## Open

- A location prompt left unanswered holds the white screen: the view waits on it.
- On the Mac the bar is full at about 3 s and the world shows at about 9 s; no
  progress is shown between, on the S23 likely longer. Not measured there.
- For a moment before the shell mounts, the page is black.
- The native app has not been run with this.
