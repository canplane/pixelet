# Working on pixelet

This is how to build pixelet, run it and check a change. How the system works is
in [ARCHITECTURE.md](./ARCHITECTURE.md).

## Setting up

Development takes a Mac: the iPhone build needs Xcode, and Safari stands in for
the iPhone's browser.

You need GNU Make, Python 3.11 or newer, [Node](https://nodejs.org/) at the
version `ts/.tool-versions` pins with pnpm (`corepack enable`), Rust through
[rustup](https://rustup.rs/) (the toolchain is pinned in
`rust/rust-toolchain.toml`), Binaryen's `wasm-opt`, Docker for the local
databases. `make install` brings [cargo-nextest](https://nexte.st/), which runs
pxl-core's unit tests one to a process, since each boots a core of its own. The
checks and the tests also need
what [xpute/README.md](./xpute/README.md#running-it) lists.

```bash
git clone git@github.com:canplane/pixelet.git
cd pixelet
make install
```

## Running the page

The page can take its tiles from three places, named the way a bakery's bread
is.

`make dev` has them **delivered**: every tile and every server route comes from
the live site, so nothing else runs here. This is the right mode for work on the
client and the core.

`make dev-oven` runs the whole stack here and bakes each tile the first time it
is asked for, which is what you want when you change a bake or a server. It
needs the local databases and an `ADMIN_TOKEN` of your choosing in
`rust/server/common/.env`:

```bash
cp infra/local/.env.example infra/local/.env
cp rust/server/common/.env.example rust/server/common/.env
docker compose -f infra/local/docker-compose.yml up -d
make dev-oven
```

`make dev-shelf` serves the tiles from the last bakes under `./out`, which is
what publishing them would show. `make preview`, `preview-oven` and
`preview-shelf` do the same over the production bundle, which is the one to
measure speed on.

The page is Expo's web build, bundled by Metro. Beside the dev server, a watcher
rebuilds the core's WebAssembly module whenever its Rust changes.

The make targets set everything a run needs. Point a run at another
deployment or other ports only by the variables `ts/client/script/proxy.ts`
reads, and run a server alone by its own target (`make serve-tile`).

A page leaves two helpers in the browser console. `xpute.cmd` sends
the core any command by its number, and `pxl.perf` is the profiler: `report()`
summarizes where the frames' time went and `trace()` saves a trace for
ui.perfetto.dev.

## Running on a device

The device app takes every route from the live site. Its native projects are
generated, not kept, and the core is compiled as a step of the native build.

On Android, with JDK 17 and the NDK installed:

```bash
make install
(cd ts/client && pnpm exec expo prebuild --platform android --no-install --clean)
(cd ts/client/android && ./gradlew :app:installRelease)
```

On iOS, generate the project with `--platform ios`, run `pod install` in
`ts/client/ios`, and build `pixelet.xcworkspace` in Xcode. `make dev-native`
starts Metro for a Debug build on either platform.

## Making data

Every kind of tile is baked by its own target (`terrain`, `place`, `division`
and `earth`), and what a bake makes goes under `./out`. A bake goes through its
steps, the finest tiles first and then the folds, and one shard can be baked a
step at a time:

```bash
make bake-terrain ARGS="--shard 01033210 --step leaf"
make bake-terrain ARGS="--shard 01033210 --step fold --step commit"
```

`make bake-star` and `make bake-geoid` are not tile sets: they make the star
catalog the sky draws and the geoid table that GPS heights are corrected by.
`make ingest-overture` fills the local geographic database, and
[infra/geo/README.md](./infra/geo/README.md) describes each source's ingest.

After a re-ingest or a change to a bake, empty what it made by hand
(`rm -rf tmp/oven`, or `out/<kind>*` to bake one kind from nothing).

## Checking a change

```bash
make -j8 check
make -j8 test
```

`check` and `test` hold pixelet's code; each also has a target per tree, such
as `check-ts` or `test-rust`. `./script/diff-check.py [base]` runs only the
checks and tests of what changed since `base` reaches (the working tree's
changes by default), and `--list` names them without running them. A deploy
adds `check-build`, the bundle a deploy builds; xpute's own `check-xpute` and
`test-xpute`; and
`test-gpu`, the renderer's passes on this machine's GPU. A change to xpute's GPU
encoder or runtime also takes `(cd xpute/rust && ../../script/perf/mac.py cargo test-gpu) && script/perf/mac.py pnpm --dir xpute/ts test:gpu:web`, which needs a GPU and
Chrome. `make fmt` formats.

`make check` also lints Android, with the newest NDK installed
(`script/env.sh`); `CC_aarch64_linux_android` and `AR_aarch64_linux_android` name
another.

Code generated from the JSON under `spec/` is never edited by hand: change the
JSON and run `make spec`.

## Deploying

A push to the `deploy` branch rebuilds the servers, publishes the client and
smoke-tests the site. [infra/README.md](./infra/README.md) has the production
setup and the same steps by hand.
