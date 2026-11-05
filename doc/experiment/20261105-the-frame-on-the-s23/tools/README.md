# Tools

How the journal's S23 runs were taken. Data and shots go under `out/s23/` at the
repository's root (gitignored).

## A tree on the phone

Each tree is a worktree under `.claude/worktrees/`, its guest built
(`pnpm build:wasm` in `ts/client`, with the session's cargo target), and served
by its own Metro as a dev bundle:

```sh
cd .claude/worktrees/<tree>/ts/client
EXPO_NO_TYPESCRIPT_SETUP=1 CI=1 ./node_modules/.bin/expo start --web --port <port>
```

Take a port nothing listens on (`lsof -nP -iTCP:<port> -sTCP:LISTEN`). With
`PXL_DEV_SOURCE=shelf` the tiles and assets come from the local shelf instead of
plei.me, after `cargo serve-tile --shelf <repo>/out` (from `rust/`, under
`script/with-env.sh`; give the path whole). The shelf's sets must be the format
the page asks (`out/terrain/manifest.json`).

## One run

`s23.mjs` loads a page on the S23's Chrome over adb, settles, and runs the
scenes asked. Its header lists them and the settings it takes. For example:

```sh
VPORT=8194 LABEL=x SCENES=off,on,drive node s23.mjs
```

## A comparison

`ab.sh` alternates trees, a fresh load each. It refuses to run where a port's
server is not the tree named:

```sh
SCENES=walk,on HOUR=morning ./ab.sh rv 2 8194:<repo>/.claude/worktrees/base 8195:<repo>/.claude/worktrees/change
python3 -I show.py light_receivers,light_beam,world base:rv1-8194,rv3-8194 change:rv2-8195,rv4-8195
```

`trace.py` reads a profiler trace that `SCENES=…,trace` saved.

## What a run must keep clear of

- One sitting drifts as the phone warms; compare runs within an alternation.
  `therm` lines in a run's log give the thermal status and the AP's temperature.
- The strata's textures land at their own time and change `world` by several ms.
  Settle before timing and look at the `-settled.png` shot.
- `pxl.perf.drive` sets `res` to fixed itself.
- The `perf` setting does not stop the timestamps' read; it only shows the panel.
- A CDP device-scale override changes `devicePixelRatio`, not the canvas.
