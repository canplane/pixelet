# Working on pixelet

This is how to build pixelet, run it and check a change. How the system works is
in [ARCHITECTURE.md](./ARCHITECTURE.md).

## Setting up

Development takes a Mac: the iPhone build needs Xcode, and Safari stands in for
the iPhone's browser.

You need:

- GNU Make and Python 3.11 or newer;
- [Node](https://nodejs.org/) at the version `ts/.tool-versions` pins, with
  pnpm, which runs the version `ts/package.json` names in `packageManager`;
- Rust through [rustup](https://rustup.rs/), which takes the toolchain
  `rust/rust-toolchain.toml` pins;
- Binaryen's `wasm-opt`;
- Docker, for the local databases;
- for the checks and the tests, what
  [xpute/CONTRIBUTING.md](./xpute/CONTRIBUTING.md) lists.

`make install` brings each tree's packages, the package the Python scripts read
the specs with (`script/requirements.txt`) and
[cargo-nextest](https://nexte.st/), which runs pxl-core's unit tests one to a
process: the core keeps its state in the process's globals, and each test boots
a core of its own.

```bash
git clone git@github.com:canplane/pixelet.git
cd pixelet
make install
```

## Running on the web

The page can take its tiles from three places, named the way a bakery's bread
is.

`make dev` has them **delivered**: every tile and every server route comes from
the live site, so no server runs here. This is the mode for work on the client
and the core.

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

Each dev target rebuilds the core's WebAssembly module when its Rust changes;
reload the page to take it. The make targets set everything a run needs. Point
a run at another deployment or other ports only by the variables
`ts/client/script/proxy.ts` reads, and run a server alone by its own target
(`make serve-tile`). How the web and native builds are made is in
[ts/client/README.md](./ts/client/README.md), "Builds".

A page leaves a helper in the browser console. `pxl.perf` is the profiler:
`report()` summarizes where the frames' time went and `trace()` saves a trace
for ui.perfetto.dev. `pxl.view.at(x, y)` says what is drawn at a point of the
view: the cell the GPU's ray met there, with its
asset, face, zoom and distance, and how the world answers that cell.

## Running natively

The native app takes every route from the live site. Its native projects are
generated, so a build starts from `expo prebuild`.

On Android, with a JDK from 17 up and the NDK installed:

```bash
make install
(cd ts/client && pnpm exec expo prebuild --platform android --no-install)
(cd ts/client/android && ./gradlew :app:installRelease)
```

On iOS, generate the project with `--platform ios`, run `pod install` in
`ts/client/ios`, and build `pixelet.xcworkspace` in Xcode. `make dev-native`
starts Metro for a Debug build on either platform.

What a change takes before it is in a build:

- under `rust/`, or to a native module's `build.gradle`, another build: the
  build compiles the core itself, as a step of xpute's native module, and
  Gradle reads a module's script where it lies;
- to a podspec, `pod install`;
- to a config plugin (`ts/client/plugin/`), the project generated again with
  `--clean` and then `pod install`: a plugin leaves what it already wrote.
  `--clean` removes `ios/` or `android/` whole, anything put there by hand
  with it.

## Making data

Every kind of tile is baked by its own target (`terrain`, `place`, `division`
and `imagery`), and what a bake makes goes under `./out`. A bake goes through its
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

After a change of source, code or coverage, purge the kind's set
(`rm -rf out/<kind> out/<kind>.stage out/<kind>.publish`) and bake every shard;
after a re-ingest or a change to a bake, empty the dev tile server's cache too
(`rm -rf tmp/oven`).

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
`test-xpute`; and `test-gpu`, the GPU's crates and the renderer's passes on
this machine's GPU. `make fmt` formats. `check-ts` holds the TypeScript to knip
too (`pnpm --dir ts knip`), which names the files, exports and dependencies
nothing reaches.

A change that touches a GPU call (`spec/gpu/`, the GPU's crates under
`rust/gpu/` and its package under `ts/gpu/`, or how pxl-render encodes) also
takes this, which needs a GPU and Chrome (`$CHROME`):

```bash
(cd rust && cargo test-gpu-host)
script/perf/mac.py pnpm --dir ts test:gpu:web
```

`make check` also lints Android, with the newest NDK installed
(`script/env.sh`); `CC_aarch64_linux_android` and `AR_aarch64_linux_android`
name another.

Code generated from `spec/` is never edited by hand: change the spec and run
`make spec`.

## Deploying

A push to the `deploy` branch rebuilds the servers, publishes the client and
smoke-tests the site. [infra/README.md](./infra/README.md) has the production
setup and the same steps by hand.
