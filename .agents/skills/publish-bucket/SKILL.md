---
name: publish-bucket
description: Upload baked tiles, ingested models and textures to the R2 bucket the Worker serves, deploy, and verify the served bytes. Use after a bake or an asset change, when the client must point at a new version, or when a live tile decodes wrong.
---

# Publishing the bucket

One private R2 bucket, `pixelet-tiles`, holds everything the client fetches
that is not the app itself. One Cloudflare Worker (`apps/web/worker/index.ts`,
`apps/web/wrangler.jsonc`) serves the built app and reads the bucket under
`/geo/tiles/*` and `/asset/*` on the app's own hostname — same-origin, no CORS, and the
client's paths stay relative. Producing what goes in is
[bake-tiles](../bake-tiles/SKILL.md) and `deno task ingest:asset`; this is
how it gets there and how you know it arrived.

## What the bucket holds

| Prefix                  | Object                       | Named by            | Policy                                                         |
| ----------------------- | ---------------------------- | ------------------- | -------------------------------------------------------------- |
| `terrain/<version>/`    | gzipped XTP page, `_ocean`   | `page_idx`          | immutable; a re-bake is a new version                          |
| `poi/<version>/`        | gzipped XTP page             | `page_idx`          | version = format; content rewritten in place monthly           |
| `geocode/<version>/`    | gzipped JSON page            | `page_idx`          | same as poi                                                    |
| `asset/`                | glb, png, webp — not gzipped | sha256 of the bytes | immutable by construction; unreferenced keys deleted whenever  |
| `_meta/land_pages.json` | terrain's land/ocean list    | fixed name          | the one mutable named object; rewritten with a terrain re-bake |

The versions the client requests are `PUBLISHED_VERSION` in
`packages/pxl/shared/src/bake/published.ts`; the asset keys are the
generated table `packages/pxl/shared/src/asset/ingested.ts`. The object
name carries no extension, so **content-type and content-encoding are
stored with the object at upload** — that is what every flag below is for.

Immutability is the whole cache story: the edge, the browser and any
future OPFS cache are invalidated by requesting a new prefix, with no purge
call. POI and geocode are the exception, and pay for it with a one-day
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

### Terrain

```bash
aws s3 sync ./out/terrain/<version> s3://pixelet-tiles/terrain/<version> \
  --profile r2 --endpoint-url $R2 --exclude "*.part" \
  --content-type application/octet-stream --content-encoding gzip \
  --cache-control "public, max-age=31536000, immutable"

aws s3 cp ./out/terrain/land_pages.json s3://pixelet-tiles/_meta/land_pages.json \
  --profile r2 --endpoint-url $R2 --content-type application/json
```

**Never overwrite an existing terrain version in place** — publish a new
prefix and point the app at it. The second command matters when the
land/ocean classification was rebuilt: the monthly workflow bakes POI and
geocode against this object, not against a local terrain bake.

**Objects are stored already gzipped, so the upload must set
`Content-Encoding: gzip`.** Without it the client receives compressed bytes
it does not know to decode. This is the single easiest step to get wrong,
and it fails as a decode error far from its cause.

### POI and geocode

```bash
aws s3 sync ./out/poi/<version> s3://pixelet-tiles/poi/<version> \
  --profile r2 --endpoint-url $R2 --exclude "*.empty" --exclude "*.part" \
  --content-type application/octet-stream --content-encoding gzip \
  --cache-control "public, max-age=86400" --delete

aws s3 sync ./out/geocode/<version> s3://pixelet-tiles/geocode/<version> \
  --profile r2 --endpoint-url $R2 --exclude "*.empty" --exclude "*.part" \
  --content-type application/json --content-encoding gzip \
  --cache-control "public, max-age=86400" --delete
```

`--exclude "*.empty"` because the bake leaves a local marker per empty page
that must not become an object; `--delete` because a refresh can remove a
page's POI and the stale object would otherwise outlive it. These are the
exact commands `.github/workflows/overture-monthly-refresh.yml` runs — keep
the two in step.

### Assets

```bash
deno task ingest:asset     # -> ./out/asset/{glb,png,webp}/<sha256>, and regenerates
                           #    packages/pxl/shared/src/asset/ingested.ts

for t in glb:model/gltf-binary png:image/png webp:image/webp; do
  aws s3 sync ./out/asset/${t%%:*} s3://pixelet-tiles/asset \
    --profile r2 --endpoint-url $R2 \
    --content-type ${t#*:} --cache-control "public, max-age=31536000, immutable"
done
```

One sync per type so the content-type is stored with the object. Not
gzipped — glb, png and webp are already compressed — so no
`Content-Encoding`. A re-run uploads only new hashes.

**Commit the regenerated `ingested.ts` with the asset change, and upload
before deploying**: the shipped palette reads that table, and a key it
names that is not in the bucket is a 404 on the live site. `deno task
deploy`'s bundle check fails if a model or image byte is back in the bundle.

## Verify what is stored

```bash
# object count should match the bake's manifest `objects`
aws s3 ls s3://pixelet-tiles/terrain/<version>/ --profile r2 --endpoint-url $R2 --recursive | wc -l

# the shared ocean tile must exist
aws s3api head-object --bucket pixelet-tiles --key terrain/<version>/_ocean --profile r2 --endpoint-url $R2

# metadata must have survived: expect ContentEncoding "gzip" on a page,
# ContentType model/gltf-binary and no ContentEncoding on a glb
aws s3api head-object --bucket pixelet-tiles --key terrain/<version>/<page_idx> --profile r2 --endpoint-url $R2
aws s3api head-object --bucket pixelet-tiles --key asset/<sha256> --profile r2 --endpoint-url $R2
```

## Deploy

Bump `PUBLISHED_VERSION` if a version changed, then:

```bash
deno task deploy
```

Through the task, not around it: it runs `check:bundle` between the build
and the publish, which is what stops a dev-only surface from reaching the
live site, and it drives wrangler through Deno rather than a system Node
that may be older than wrangler requires. What the Worker is and how it
routes is [infra/README.md](../../../infra/README.md) §6, along with the
one-loop smoke test over every path the app takes.

## Verify what is served

**Check the served bytes, not just the stored ones.** The bucket can be
perfectly correct while the serving path is not:

```bash
U=https://plei.me/geo/tiles/terrain/<version>/<page_idx>
curl -s -H "Accept-Encoding: gzip" -D - -o /dev/null "$U"   # expect content-encoding: gzip
curl -s --compressed "$U" | head -c 2 | xxd -p              # expect 5854 (XT), never 1f8b
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

Also confirm an unbaked ocean page returns **404 `text/plain`**, not
`index.html`: that is what a missing `run_worker_first` produces, and the
client would try to decode the HTML as terrain. An asset answers
`model/gltf-binary` or `image/png` with no `content-encoding`.

## Purging the edge cache

The Worker caches every tile it serves in `caches.default`, the zone's edge
cache, and by design never needs to purge: a terrain or asset URL never
changes meaning, and a POI/geocode entry expires within its one-day
`max-age`. Purge only to see an in-place refresh immediately, or after
fixing a serving bug whose wrong bytes are already cached.

```bash
# everything on the zone (the app's static assets are content-hashed, so this costs one cold fetch each)
curl -X POST "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/purge_cache" \
  -H "Authorization: Bearer $CF_API_TOKEN" -H "Content-Type: application/json" \
  --data '{"purge_everything":true}'

# or by URL, up to 30 per call
curl -X POST "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/purge_cache" \
  -H "Authorization: Bearer $CF_API_TOKEN" -H "Content-Type: application/json" \
  --data '{"files":["https://plei.me/geo/tiles/poi/<version>/<page_idx>"]}'
```

The token needs the `Zone → Cache Purge` permission; the zone ID is on the
zone's overview page in the dashboard, which also offers the same two
purges under Caching → Configuration. Both reach entries the Worker put
there through the Cache API. `cache.delete(request)` inside the Worker
would not: it clears the one colo that ran it.

Old version prefixes simply stop being requested; delete them whenever.
