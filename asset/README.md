# asset

The shipped tree: every model, texture and skin the client draws with. Data,
not code — nothing in the repository imports a file from here. The client
fetches each one from the bucket by the SHA-256 of its bytes.

```
asset/
├── actor/      character and companion models (Kenney)
├── cube/       the texture pack: terrain/ and block/ at 256², original/ at source size
├── decor/      trees, boats, cars, trains (Kenney)
├── place/      symbol/: the place icons (openstreetmap-carto `symbols/`, CC0)
└── sky/        the Moon's near side and the Earth's twelve months (NASA), cloud/: the cumulus (Quaternius, CC0)
```

## What to run, and when

Each is a binary of `pxl-tool` (`crates/tool`), run by a task, and
`deno task check` clippies and tests every one with the rest.

### After changing anything under `actor/`, `cube/` or `decor/`

```sh
deno task ingest:asset
```

It copies every file to `./out/asset/{glb,png,webp}/<sha256>`, composes the
sixty texture tiles into one WebP sheet, measures every model's extent out of
its glTF, and writes the two tables that name them:

- `crates/spec/src/asset/ingested.spec.rs` — what the kernel compiles
- `apps/web/src/catalog/asset/ingested.spec.ts` — what the shell compiles

**Commit both tables with the asset change, and upload before deploying.**
The shipped palette reads that table, and a key it names that is not in the
bucket is a 404 on the live site. Uploading is the `publish-bucket` skill.

`deno task test:rust` holds the committed table against the files: every
model measured again, and the row count checked. It runs in the gate.

### After changing which texture pack is used

```sh
deno task download:pack
```

It fetches the pack's sources, resizes each tile to 256², flattens alpha and
writes them into `cube/`, keeping the source-size originals under
`cube/original/`. `original/` is never ingested — the client fetches the
resized tiles, and the sheet is composed from those.

Run the ingest afterward: the sheet's key changes with any tile.

### After changing the Moon's map

```sh
deno task download:moon
```

It fetches the CGI Moon Kit's 1K color map, cuts the near side, takes it to
gray reflectance and writes `sky/moon.png`, and prints the face's mean,
which `sky.wgsl`'s `MOON_MAP_MEAN` has to equal. Run the ingest afterward.

### After changing the Earth's maps

```sh
deno task bake:earth
```

It fetches Blue Marble Next Generation's July of 2004 and the Black Marble's
lights of 2016 (NASA's Earth Observatory) and makes both forms of the Earth's
picture from them: `sky/earth/07.webp`, the global one at 2048 × 1024, and the
earth layer's pages under `out/earth/<version>/`, which publish as a bake does.
Run the ingest afterward.
