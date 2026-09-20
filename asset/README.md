# asset

The shipped tree: every model, texture and skin the client draws with. Data,
not code — nothing in the repository imports a file from here. The client
fetches each one from the bucket by the SHA-256 of its bytes.

```
asset/
├── actor/      character and companion models (Kenney)
├── cube/       the texture pack: terrain/ and block/ at 256², original/ at source size
├── decor/      trees, boats, cars, trains (Kenney)
└── script/     what turns the tree above into what the bucket holds
```

## What to run, and when

Both are single-file Rust scripts. They need a nightly toolchain
(`rustup toolchain install nightly`), which is what runs a `.rs` file
directly; nothing else about them is nightly.

### After changing anything under `actor/`, `cube/` or `decor/`

```sh
./asset/script/ingest_asset.rs
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

Run the ingest afterwards: the sheet's key changes with any tile.
