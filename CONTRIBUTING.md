# Working on pixelet

This is how to build pixelet, run it and check a change. How the system works is
in [ARCHITECTURE.md](./ARCHITECTURE.md).

## Setting up

You need GNU Make, Python 3.11 or newer, [Node](https://nodejs.org/) at the
version `ts/.tool-versions` pins with pnpm (`corepack enable`), Rust through
[rustup](https://rustup.rs/) (the toolchain is pinned in
`rust/rust-toolchain.toml`), Binaryen's `wasm-opt`, and Docker for the local
databases. The checks and the tests also need what
[xpute/README.md](./xpute/README.md#running-it) lists.

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
needs the local databases:

```bash
cp infra/local/.env.example infra/local/.env
cp rust/server/.env.example rust/server/.env
docker compose -f infra/local/docker-compose.yml up -d
make dev-oven
```

`make dev-shelf` serves the tiles from the last bakes under `./out`, which is
what publishing them would show. `make preview`, `preview-oven` and
`preview-shelf` do the same over the production bundle, which is the one to
measure speed on.

The page is Expo's web build, bundled by Metro. Beside the dev server, a watcher
rebuilds the core's WebAssembly module whenever its Rust changes. The dev server
forwards the server routes the way the Worker does, and the environment can
change where they go:

| Variable                                                    | What it sets                                                       |
| ----------------------------------------------------------- | ------------------------------------------------------------------ |
| `PXL_TILE_SOURCE`                                            | `delivery` (the default), `oven` or `shelf`, as the targets set it |
| `PXL_REMOTE_ORIGIN`                                         | the deployment that `delivery` reads from                          |
| `PXL_GEO_BASE_URL`, `PXL_TILE_BASE_URL`, `PXL_API_BASE_URL` | the local servers, `127.0.0.1` on 3000, 3001 and 3002 by default   |
| `PXL_PREVIEW_PORT`                                          | the preview's port, 4173 by default                                |

A development page leaves two helpers in the browser console. `xpute.cmd` sends
the core any command by its number, and `pxl.perf` is the profiler: `report()`
summarizes where the frames' time went and `trace()` saves a trace for
ui.perfetto.dev.

## Running on a device

The device app takes every route from the live site. Its native projects are
generated, not kept, and the core is compiled as a step of the native build.

On Android, with JDK 17 and the NDK installed:

```bash
make install
(cd ts/client && pnpm exec expo prebuild --platform android --no-install)
(cd ts/client/android && ./gradlew :app:installMobileRelease)
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
A change to a bake starts the dev tile server's cache afresh by itself. After a
re-ingest, empty what it made by hand (`rm -rf tmp/oven`, or
`out/<kind>*` to bake one kind from nothing).

## Checking a change

```bash
make -j8 check
make -j8 test
```

`check` runs every check and `test` every test; each also has a target per tree,
such as `check-ts` or `test-rust`. A change to a shader or to how the renderer
encodes its GPU calls also takes `make -C xpute test-gpu`, which needs a GPU and
Chrome. `make fmt` and `make lint` format and lint.

`make check` also lints Android, which needs the NDK's compiler at the app's
minSdk:

```bash
ndk=~/Library/Android/sdk/ndk/<version>/toolchains/llvm/prebuilt/darwin-x86_64/bin
export CC_aarch64_linux_android=$ndk/aarch64-linux-android29-clang \
  AR_aarch64_linux_android=$ndk/llvm-ar
```

Code generated from the JSON under `spec/` is never edited by hand: change the
JSON and run `make spec`.

## Deploying

A push to the `deploy` branch rebuilds the servers, publishes the client and
smoke-tests the site. [infra/README.md](./infra/README.md) has the production
setup and the same steps by hand.
