# Tools

How the S23 runs of [the record](../../README.md) were taken. Data and shots go under `cache/s23/` at the
repository's root (gitignored).

## A tree on the phone

The page is the main checkout's, its guest built (`pnpm build:wasm` in
`ts/client`, with the session's cargo target), and served by its own Metro as a
dev bundle, from the repository's root:

```sh
cd ts/client
EXPO_NO_TYPESCRIPT_SETUP=1 CI=1 ./node_modules/.bin/expo start --web --port <port>
```

A comparison's other side is a second tree, served the same way from its own
`ts/client` on another port.

`s23.js` reaches the page's modules by path. Since `2651534165`, main names
`src/services/share.ts` `src/service/share.ts` and no longer exports
`perf_frames` from `src/port/perf.ts`, so it does not run against main as it
stands.

Take a port nothing listens on (`lsof -nP -iTCP:<port> -sTCP:LISTEN`). With
`PXL_DEV_SOURCE=shelf` the tiles and assets come from the local shelf instead of
plei.me, after `cargo serve-tile --shelf <repo>/out` (from `rust/`, under
`script/with-env.sh`; give the path whole). The shelf's sets must be the format
the page asks (`out/terrain/manifest.json`).

## One run

`s23.js` loads a page on the S23's Chrome over adb, settles, and runs the
scenes asked. Its header lists them and the settings it takes. For example:

```sh
VPORT=8194 LABEL=x SCENES=off,on,drive node s23.js
```

## A comparison

`ab.sh` alternates trees, a fresh load each. It refuses to run where a port's
server is not the tree named:

```sh
SCENES=walk,on HOUR=morning ./ab.sh rv 2 8194:<base tree> 8195:<repo>
python3 -I show.py light_receivers,light_beam,world base:rv1-8194,rv3-8194 change:rv2-8195,rv4-8195
```

`trace.py` reads a profiler trace that `SCENES=…,trace` saved.

## What a run must keep clear of

- One sitting drifts as the phone warms; compare runs within an alternation.
  `therm` lines in a run's log give the thermal status and the AP's temperature.
- The strata's textures land at their own time and change `world` by several ms.
  Settle before timing and look at the `-settled.png` shot.
- `pxl.perf.drive` sets `res` to fixed itself. A baseline takes two cool runs and one
  sustained run (`DRIVE_S=300 FRAMES=1`, its last 120 s), the phone warm as a user's.
- The `perf` setting does not stop the timestamps' read; it only shows the panel.
- A CDP device-scale override changes `devicePixelRatio`, not the canvas.

## Safari's page

`measure.js` loads a page in the Mac's Safari through safaridriver (`safaridriver -p 4444`) and runs its phases: default zoom, farthest zoom with the sky time moving, back, shadows on. Every 5 s it logs the page's live GPU bytes and object count, the GPU process's footprint and the GPU's utilization to `<label>.jsonl`, and it stops if the GPU pins at 100% or the footprint passes 1.2 GB. Serve the page behind `mem-proxy.js <metro port> <port>`, which counts every buffer and texture from before the first frame, or `pass-proxy.js`, which also gives each pass its own submit and logs the passes still outstanding:

```sh
VPORT=<port> FAR_S=600 node measure.js <label>
```

A pin holds the GPU until Safari is force-quit.

## Against the deployed build (11-07)

The `cmp-*` tools take the deployed build, served by Vite from its constant tree
(`.claude/worktrees/deployed`, e317a3644b with e2b019f1b0 and b6dac91e78) on port 8291, against
the main checkout's page, served by Metro on 8298, both off the shelf on 3001:

- `cmp-main-up.sh` builds the main checkout's kernel and serves its page and the shelf.
- `script/compare-perf.py http://localhost:8291/ http://localhost:8298/ <rounds>` is the gate.
- `cmp-prof.py <dir> <tag> <url>` takes one drive's `pxl.perf.report()` on the Mac.
- `cmp-rest.py <dir> <tag> <url>` takes 8 s of the drive, then 65 s untouched, and shots.
- `cmp-hot.js <port> <label> <seconds>` takes one drive on the S23's Chrome, the GPU's clock
  every 5 s, and with `REST=65` the standing window after it; `cmp-s23.sh` runs the pages by
  turns; `cmp-s23tab.py` tabulates them with their clocks.

Each runs under the GPU lock (`lockf -k <workspace>/.gpu.lock`); their data go under cache/cmp/.
