# asset

Every model, texture and skin the client draws with, and what they are made
from. Data, not code — nothing in the repository imports a file from here.

- `content/` is what ships: the ingest's one input. A catalog names a file by
  its path under it; the client fetches it from the bucket by the SHA-256 of its
  bytes, which where it lies does not change.
- `raw/` is what came in, as it came: what a tool turns into `content/`, and
  what no ingest reads. What can surely be fetched again is not kept:
  `raw/sky/`, NASA's Moon and CDS's stars, fetched by running the tool that
  reads them. The texture pack's originals are kept, since their URLs may not
  last.

```
asset/
├── content/
│   ├── actor/   character and companion models (Kenney)
│   ├── cube/    the texture pack's tiles: terrain/ and object/ at 256²
│   ├── decor/   trees, boats, cars, trains (Kenney)
│   └── sky/     the Moon's near side (NASA), cloud/: the cumulus (Quaternius, CC0)
└── raw/
    ├── cube/    the pack's manifest and its originals at source size
    ├── place/   symbol/: the place icons (openstreetmap-carto `symbols/`, CC0), drawn into code
    └── sky/     moon/, star/: NASA's map and CDS's catalog, as fetched
```

## What to run, and when

Each is a binary of `pxl-tool` (`rust/tool`), run by make, and `make -j8 check`
clippies and tests every one with the rest.

### After changing anything under `content/`

```sh
make ingest-asset
```

It copies every file to `./out/asset/<sha256>.<ext>`, composes the texture tiles
into one WebP sheet, measures every model's extent out of its glTF, and writes
the table that names them, `spec/spec/asset/ingested.json`, and draws
`raw/place/symbol/` into the code that carries the place icons. Then it writes
what the spec generates from the table:

- `ts/client/src/namespace/ingested.spec.ts` — the host's table, a key for each
  file's number
- `ts/client/src/namespace/catalog.spec.ts` — the catalog the host plugs in,
  which file each of the kernel's slots names
- `rust/spec/src/asset/ingested.spec.rs` — the pack's layout, which the renderer
  compiles

**Commit the table and what it generates with the asset change, and upload
before deploying.** The shipped palette reads that table, and a key it names
that is not in the bucket is a 404 on the live site. Uploading is the
`publish-bucket` skill.

### After changing which texture pack is used

```sh
make download-pack
```

It fetches each entry of `raw/cube/manifest.json` into `raw/cube/` at source
size, resizes it to 256², flattens alpha and writes it into `content/cube/`. The
sheet is composed from those.

Run the ingest afterward: the sheet's key changes with any tile.

### After changing the Moon's map

```sh
make download-moon
```

It fetches the CGI Moon Kit's 1K color map into `raw/sky/moon/`, cuts the near
side, takes it to gray reflectance and writes `content/sky/moon.png`, and prints
the face's mean, which pxl-render frame/shader/word.rs `MOON_MAP_MEAN` has
to equal. Run the ingest afterward.

The Earth's picture is not here: it is the earth layer's, its source in
`infra/earth/` and both its forms, the tiles and the picture whole, in the
layer's set, `out/earth/`.
