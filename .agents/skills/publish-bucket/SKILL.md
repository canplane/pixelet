---
name: publish-bucket
description: Upload baked tiles, ingested models and textures to the R2 bucket the Worker serves, deploy, and verify the served bytes. Use after a bake or an asset change, when the client must point at a new version, or when a live tile decodes wrong.
---

# Publishing the bucket

One private R2 bucket, `pixelet-tiles`, holds everything the client fetches
that is not the app itself. One Cloudflare Worker (`apps/web/cloudflare-worker/index.ts`,
`apps/web/wrangler.jsonc`) serves the built app and reads the bucket under
`/tile/*` and `/asset/*` on the app's own hostname — same-origin, no CORS, and the
client's paths stay relative. Producing what goes in is
[bake-tiles](../bake-tiles/SKILL.md) and `deno task ingest:asset`; this is
how it gets there and how you know it arrived.

## What the bucket holds

| Prefix                | Object                      | Named by               | Policy                                                        |
| --------------------- | --------------------------- | ---------------------- | ------------------------------------------------------------- |
| `terrain/<version>/`  | packed XTP tile             | name, 5–15             | immutable; a re-bake is a new version                         |
| `place/<version>/`    | XTP count tile              | name, 5–15             | immutable; a re-bake is a new version                         |
| `division/<version>/` | XTP division tile           | name, 5–12             | immutable; a re-bake is a new version                         |
| `asset/`              | glb, png, webp — not packed | its path, hash in stem | immutable by construction; unreferenced keys deleted whenever |
| `_meta/build-counter` | the official build count    | fixed name             | mutable; taken and written by CI's gate, never by hand        |

A tile is named by its 2D Morton key at the zoom the tile is held at, one
base-4 digit a zoom, coarsest first, rows counted from the south (pxl-spec
space/hierarchy/morton.rs `write_name`), on disk as in the bucket. The length is the zoom, so nothing writes a zoom down, and
the three bakes differ only in how long a name they have — terrain and places 5
to 15 digits for the pyramid they share, divisions 5 to 12, since its leaves are
z12. They land in one space, so
`terrain/<v>/01033210`, `place/<v>/01033210` and `division/<v>/01033210` are the
same ground, and **one prefix is one region across all three at every zoom**.
That is what a later split of the bucket would be: a prefix match, not a
second re-keying.

The versions the client requests are `PUBLISHED_VERSION` in
`spec/spec/bake/published.json`; the asset keys are the generated table
`apps/web/src/catalog/asset/ingested.spec.ts`. The object
name carries no extension, so **content-type and content-encoding are
stored with the object at upload** — that is what every flag below is for.

Immutability is the whole cache story: the edge, the browser and any
future OPFS cache are invalidated by requesting a new prefix, with no purge
call. Places and divisions are the exception, and pay for it with a one-day
`max-age` instead of a year.

## One-time setup

Done in the Cloudflare dashboard, not from here:

1. Create the bucket. Its name must match `r2_buckets[0].bucket_name` in
   `wrangler.jsonc`. Keep it **private** — the Worker reads it through a
   binding and it never needs to be publicly readable.
2. Create an R2 API token (S3-compatible Access Key ID + Secret).
3. Note the account ID; the S3 endpoint is
   `https://<account_id>.r2.cloudflarestorage.com`.

Store the credentials in a dedicated AWS CLI profile so an existing default
profile is left alone, and export the endpoint once per shell:

```bash
aws configure --profile r2          # region: auto
export R2=https://<account_id>.r2.cloudflarestorage.com
```

`aws s3 sync` rather than `rclone` because the AWS CLI is already present;
either works, R2 is S3-compatible. `wrangler r2 object put` is one object
per call and unusable at this scale. If the CLI rejects an upload over
checksums (v2 sends `x-amz-checksum-*` headers R2 refuses), set
`AWS_REQUEST_CHECKSUM_CALCULATION=when_required`.

## Upload

**What a publish costs is one Class A operation an object**, and a server-side
copy is the same one. The free month has a million, and nothing stops at it:
the guarantee that the bucket cannot bill covers its reads, which all pass
the Worker's fail-closed plan, and not its writes. A new terrain version is
every tile of the pyramid — the sum of `objects` over the bake manifest's
`zooms`, 227,310 for v6 — and a places version likewise, so four terrain
publishes in a month, or one rename done twice, is past it. September 2026
went to 1.67M on bakes re-published and the old tile rename run twice.

So before an upload, and after one:

```bash
deno task usage     # Class A this month against the million, and the rest
```

and set the upload's object count against what is left. A sync re-run
after an interruption is cheap — it skips an object already there — and a
version costs a pyramid's worth. Deleting an old version is free.

A version is uploaded from `out/<kind>.publish/`, which the bake's
`publish` stage makes from the set (`tile/shard.rs` `publish`): every tile,
less for places and divisions those no reader asks for. It is a copy for the
upload alone. `deno task publish <kind>…` (`script/publish.ts`) uploads it
under a new name, `v<format>.<UTC day>.<n>` — the format is
`spec/spec/bake/published.json`'s `FORMAT`, set by hand, and n is past every
version of that format and day the bucket holds — checks the stored count,
sets `PUBLISHED_VERSION`, regenerates, and removes the copy.

### Terrain

```bash
deno task bake:terrain --stage publish
deno task publish terrain
```

**Never overwrite an existing version in place**, of any kind — publish a new
prefix and point the app at it.

**Objects are stored already packed, so the upload sets the
`Content-Encoding` they were packed in** — the manifest's `content_encoding`,
which the bake writes beside the version: zstd from the bake after v4, gzip
for terrain v4, places v5 and divisions v2, which were baked before it. The
browser decodes by that header, and the kernel and the shell never see the
codec (decision/20260913-compression-by-traffic.md). A wrong label hands the
client compressed bytes it does not know to decode. This is the single
easiest step to get wrong, and it fails as a decode error far from its
cause. A bake refuses to add objects to a directory of the other encoding,
so one version holds one encoding.

### Places

As terrain:

```bash
deno task bake:place --stage publish
deno task publish place
```

A tile its parent's occupancy marks none is no object — the tree stops where no cell over a
tile holds two places — and the client reads its ground from the tile over it,
so a missing tile is never an error to repair. The client shows places only outside production until
the layer is switched on there.

### Divisions

As terrain:

```bash
deno task bake:division --stage publish
deno task publish division
```

Empty markers are local.

### Assets

```bash
deno task ingest:asset
# -> ./out/asset/<path>/<stem>-<12 hex>.<ext>, the tree the bucket holds, and
#    regenerates crates/spec/src/asset/ingested.spec.rs and
#    apps/web/src/catalog/asset/ingested.spec.ts

for t in glb:model/gltf-binary png:image/png webp:image/webp; do
  aws s3 sync ./out/asset s3://pixelet-tiles/asset \
    --profile r2 --endpoint-url $R2 \
    --exclude "*" --include "*.${t%%:*}" \
    --content-type ${t#*:} --cache-control "public, max-age=31536000, immutable"
done
```

One sync per type so the content-type is stored with the object, selected by
extension now that the tree mirrors `asset/` rather than being split by type.
Not gzipped — glb, png and webp are already compressed — so no
`Content-Encoding`. A re-run uploads only what changed, since a changed file
has a new name.

**Commit both regenerated tables with the asset change, and upload
before deploying**: the shipped palette reads that table, and a key it
names that is not in the bucket is a 404 on the live site. `deno task
deploy`'s bundle check fails if a model or image byte is back in the bundle.

## Verify what is stored

```bash
# object count should match the files in out/<kind>.publish
aws s3 ls s3://pixelet-tiles/terrain/<version>/ --profile r2 --endpoint-url $R2 --recursive | wc -l

# metadata must have survived: expect the manifest's ContentEncoding on a tile,
# ContentType model/gltf-binary and no ContentEncoding on a glb
aws s3api head-object --bucket pixelet-tiles --key terrain/<version>/<name> --profile r2 --endpoint-url $R2
aws s3api head-object --bucket pixelet-tiles --key asset/<sha256> --profile r2 --endpoint-url $R2
```

## Deploy

With `PUBLISHED_VERSION` set by the publish:

```bash
deno task deploy
```

Through the task, not around it: it runs `check:bundle` between the build
and the publish, which is what stops a dev-only surface from reaching the
live site, and it drives wrangler through Deno rather than a system Node
that may be older than wrangler requires. What the Worker is and how it
routes is [infra/README.md](../../../infra/README.md) §4, along with the
one-loop smoke test over every path the app takes.

## Verify what is served

**Check the served bytes, not just the stored ones.** The bucket can be
perfectly correct while the serving path is not:

```bash
U=https://plei.me/tile/terrain/<version>/<name>
curl -s -H "Accept-Encoding: gzip, zstd" -D - -o /dev/null "$U"   # expect the manifest's content-encoding
curl -s -H "Accept-Encoding: gzip, zstd" "$U" | zstd -dcq | head -c 2 | xxd -p   # zstd: expect 5854 (XT)
curl -s --compressed "$U" | head -c 2 | xxd -p                  # gzip: expect 5854 (XT), never 1f8b
```

A response whose gzip body is _smaller_ than the stored object means the
runtime compressed it a second time — `encodeBody: "manual"` is missing
from the Worker's `Response`. A body that still starts `1f8b` after
`--compressed` means the same thing from the other direction.

**Request each tile twice.** Edge caching sits in front of R2, and
`encodeBody: "manual"` does not survive the cache round trip — a first
request can be correct while every cached hit after it is double-compressed.
Compare both responses against the stored size, and check `cf-cache-status`
to confirm the second one actually hit.

Also confirm an unbaked ocean tile returns **204** with no body, not
`index.html`: that is what a missing `run_worker_first` produces, and the
client would try to decode the HTML as terrain. A 404 is for a key that names
nothing, and the client takes it as a failure. An asset answers
`model/gltf-binary` or `image/png` with no `content-encoding`.

## Purging the edge cache

The Worker caches every tile it serves in `caches.default`, the zone's edge
cache, and by design never needs to purge: a bake's or an asset's URL never
changes meaning. Purge only after fixing a serving bug whose wrong bytes are
already cached.

```bash
# everything on the zone (the app's static assets are content-hashed, so this costs one cold fetch each)
curl -X POST "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/purge_cache" \
  -H "Authorization: Bearer $CF_API_TOKEN" -H "Content-Type: application/json" \
  --data '{"purge_everything":true}'

# or by URL, up to 30 per call
curl -X POST "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/purge_cache" \
  -H "Authorization: Bearer $CF_API_TOKEN" -H "Content-Type: application/json" \
  --data '{"files":["https://plei.me/tile/place/<version>/<name>"]}'
```

The token needs the `Zone → Cache Purge` permission; the zone ID is on the
zone's overview page in the dashboard, which also offers the same two
purges under Caching → Configuration. Both reach entries the Worker put
there through the Cache API. `cache.delete(request)` inside the Worker
would not: it clears the one colo that ran it.

Old version prefixes simply stop being requested; delete them whenever.
