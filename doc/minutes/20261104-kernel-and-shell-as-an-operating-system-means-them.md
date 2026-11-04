# Kernel and shell, as an operating system means them

**2026-11-04** — recorded with the user. Main was at `e82319435`. The user asked
why agents mix host and guest, kernel and shell. The comments beside the
brackets name the shell for what the host does (the memory given, the directory
read, `run` rung), and port 255, `SHELL`, carried the shell and the drivers
alike, so one word meant the UI in `shell/` and the whole client on the rings.

## What the user decided

- **Kernel and shell mean what they mean in an operating system**, read no
  further than that: the kernel is the core, the shell the UI. In friendlier
  words, core and UI.
- **A port is a scheduling unit.** What runs on one loop shares one port; the
  shell and what the platform's sensors, GPU and visibility tell run on the
  platform's event loop, so they share 255. Splitting them by role would give
  two numbers to what talks directly anyway.
- **1 and 255 are pixelet's names**, not xpute's. To xpute's host every port
  but 0 is the guest's; 0 is the host's control plane.
- **The core's way in and out is its ABI alone.** A sensor or an actuator means
  nothing to it; how raw the ABI is, a sensor's own terms or a cut at the
  level the core needs, is how the core is written.
- **`driver/` mixed two things, so it goes.** What answers the kernel's
  hostcalls and what the shell's port carries are apart.
- **The layout follows the naming rules and the web's usual words**, as the
  session laid it out and the user took it.

## What was weighed and not taken

- `CLIENT` for 255: it reads as a server's client.
- `USERLAND` for 255: an operating system's line between kernel and user land
  is privilege, and here it runs between the host, 0, and every guest, the
  kernel among them.
- The shell as everything around the kernel, its devices too (functional core,
  imperative shell): no operating system reads a shell that way.
- `PLATFORM`, `PLACE`, `OUTSIDE`, `LOOP` and `CORE` as port names: with 255 the
  UI, `KERNEL` and `SHELL` stand.

## What was built

- `ts/client/src/sys/`, what answers the kernel's hostcalls, as `cfg.sys`
  hands them to `@xpute/host`: `sys/fs/` the files (the namespace, its
  backends, the source's declaration, the refusals, the catalog), `sys/text.ts`
  and `sys/stop.ts`.
- `ts/client/src/origin/`, the server the client reads from: the routes, the
  published tables, the ingest's keys and whether the device is online.
- `ts/client/src/port/`, a file a major on the shell's port, now as the door's
  (`door/apply/`): `fs.ts` (`FS_MOUNT`, `FS_TRUNCATED`, sent for `sys/fs/`),
  `gpu.ts`, `sys.ts` (`SYS_SLEEP`), `surface.ts` with each turn's frame, and
  `sensor.ts` with the readings and the kernel's asks to start and stop.
- The generators write into the new paths (`rust/tool/src/spec/`); the spec's
  docs, the kernel's comments, `AGENTS.md`, `ARCHITECTURE.md`, `README.md` and
  `asset/README.md` say `sys/`, `port/` and `origin/`, and the kernel's
  comments say the client where they said the driver.

## Open

- The bracket comments that name the shell for the host's work:
  `rust/client-bracket/wasm/src/lib.rs`, `wasm/build.rs`, `wasm/Cargo.toml`,
  `native/src/lib.rs`, `native/include/pxl_native.h`, and
  `spec/client/kernel/sys/mem/mem.json`, which writes `mem.spec.rs`.
- The kernel's sensor ABI stays raw: it takes the platform's permissions, its
  gating and its error codes.
- Checked by format, lint and type (`pnpm --dir ts check`) and by `cargo spec`;
  the tests were not run.
