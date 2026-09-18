# tool

What turns the shipped asset tree into the objects the bucket holds, and what
puts the elevation raster into db-geo. Run by hand a few times a year, never by
a server — which is why this is a root of its own rather than an `apps/`
member: nothing under it is deployed.

```
tool/
├── ingest/asset/   asset/ tree → out/asset/{glb,png,webp}/<sha256> and the table of their keys
└── elevation/      GEDTM30 → db-geo (bash + GDAL)
```

**What fills db-geo from Overture is not here.** The five ingests are binaries
of `crates/pxl/geo-bake`, the crate that bakes from those tables — one
pipeline, one connection, one catalog: `deno task ingest:{base,buildings,
transportation,places,divisions}`. The three page bakes are the same crate's:
`bake:tiles`, `bake:poi`, `bake:geocode`
([the server in Rust](../issue/20260918-the-server-in-rust-and-the-line-already-drawn.md)).

The asset ingest is still Deno because it is `sharp`'s, and elevation is bash
and GDAL and was never TypeScript.

Each tool's flags are in its own header.

The procedures — what to bring up, what order, what to check, how to upload and
verify what is served — are not here:

- filling db-geo (the ingests): [infra/README.md](../infra/README.md) §2–4
- baking terrain, POI, geocode: [.agents/skills/bake-tiles](../.agents/skills/bake-tiles/SKILL.md)
- uploading, deploying, verifying, purging: [.agents/skills/publish-bucket](../.agents/skills/publish-bucket/SKILL.md)
- the monthly POI/geocode refresh, unattended: `.github/workflows/overture-monthly-refresh.yml`
- why it is shaped this way: [issue/20260816-deployment-hosting.md](../issue/20260816-deployment-hosting.md)
