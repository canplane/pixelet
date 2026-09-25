# asset

The shipped tree: every model, texture and skin the client draws with. Data,
not code — nothing in the repository imports a file from here. The client
fetches each one from the bucket by the SHA-256 of its bytes.

```
asset/
├── actor/      character and companion models (Kenney)
├── cube/       the texture pack: terrain/ and block/ at 256², original/ at source size
├── decor/      trees, boats, cars, trains (Kenney)
├── sky/        the Moon's near side (NASA's Scientific Visualization Studio)
└── script/     what turns the tree above into what the bucket holds
```

## What to run, and when

One is a crate and one is a script, and they run differently. The ingest is
`asset/script/Cargo.toml`'s only binary, on the pinned toolchain, and
`deno task check` clippies it with the rest. The texture-pack download
stays a single file on nightly (`rustup toolchain install nightly`): it is
run by hand, and its HTTP client would otherwise be compiled by every gate
for nothing.

### After changing anything under `actor/`, `cube/` or `decor/`

```sh
cargo run --manifest-path asset/script/Cargo.toml
```

It copies every file to `./out/asset/{glb,png,webp}/<sha256>`, composes the
sixty texture tiles into one WebP sheet, measures every model's extent out of
its glTF, and writes the two tables that name them:

- `crates/pxl/spec/src/asset/ingested.spec.rs` — what the kernel compiles
- `apps/web/src/catalog/asset/ingested.spec.ts` — what the shell compiles

**Commit both tables with the asset change, and upload before deploying.**
The shipped palette reads that table, and a key it names that is not in the
bucket is a 404 on the live site. Uploading is the `publish-bucket` skill.

`deno task test:asset` holds the committed table against the files: every
model measured again, and the row count checked. It runs in the gate.

### After changing which texture pack is used

```sh
./asset/script/download_texture_pack.rs
```

It fetches the pack's sources, resizes each tile to 256², flattens alpha and
writes them into `cube/`, keeping the source-size originals under
`cube/original/`. `original/` is never ingested — the client fetches the
resized tiles, and the sheet is composed from those.

Run the ingest afterward: the sheet's key changes with any tile.

### After changing the Moon's map

```sh
./asset/script/download_moon.rs
```

It fetches the CGI Moon Kit's 1K color map, cuts the near side, takes it to
grey reflectance and writes `sky/moon.png`, and prints the face's mean,
which `sky.wgsl`'s `MOON_MAP_MEAN` has to equal. Run the ingest afterward.
