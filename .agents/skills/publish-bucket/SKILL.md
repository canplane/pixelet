---
name: publish-bucket
description: Upload baked tiles, ingested models and textures to the R2 bucket the Worker serves, and verify the served bytes. Use after a bake or an asset change, when the client must point at a new version, or when a live tile decodes wrong.
---

# Publishing the bucket

How a tile set or an asset gets into the bucket `pixelet-tiles`, and how to
confirm it arrived. What the bucket holds and how the Worker serves it is
[infra/README.md](../../../infra/README.md) §4; building a set is
[bake-tiles](../bake-tiles/SKILL.md).

Only `./script/publish.py` writes the bucket's `source` object, which the
client reads at boot: publishing needs no commit or deploy. **A manual upload
sets content-type and content-encoding.**

## One-time setup

In the Cloudflare dashboard, create the bucket with the name in
`ts/server/edge/wrangler.jsonc` (`r2_buckets[0].bucket_name`) and keep it
**private**; create an R2 API token (an S3-compatible key ID and secret); note
the account ID. Then make a separate AWS profile:

```bash
aws configure --profile r2          # region: auto
aws configure set endpoint_url https://<account_id>.r2.cloudflarestorage.com --profile r2
```

`publish.py` uses that profile (`$R2`, if set, overrides its endpoint). A manual
`aws s3 sync` also needs `AWS_REQUEST_CHECKSUM_CALCULATION=when_required`.

## Upload

**Each uploaded object is one Class A operation, and nothing stops writes at
the free million a month**; a terrain version is every tile. Check usage before
and after, against the object count `publish` prints:

```bash
./.agents/skills/publish-bucket/usage.py     # Class A this month against the million
```

Deleting an old version is free.

```bash
make bake-<kind> ARGS="--stage publish"    # terrain, place or division
./script/publish.py <kind>
```

`publish` names the version, uploads with the manifest's content-encoding,
checks the stored count, then writes `source`: `upload` and `serve` at once.

```bash
./script/publish.py upload <kind>                       # up as a new version, served by no one
./script/publish.py serve <kind> <version>              # serve an older version again: how to roll back
./script/publish.py override <shard> <kind> <version>   # one shard and the tiles under it; - drops it
```

**Never overwrite an existing version**, of any kind: publish a new one and
serve it.

A missing place or division tile is not an error.

### A new format

The plain `publish.py <kind>` is for a version in the format the deployed
client reads. A client takes `source` only where every kind is in its own
format exactly (pxl-kernel `sys/io/source.rs`, `reads_every`), and
`declare_source` judges by the tree's format, not the deployed client's, so it
lets through a format no deployed client reads. A version in a new format is
uploaded with `upload` and served with `serve` when the client that reads it
is out. Under that exact match, whichever goes first, the clients of the other
format refuse `source` until they are replaced: the switch interrupts one
side. Whether that is taken, or avoided some other way, is the user's call.

### Earth

```bash
make download-earth
make bake-earth
./script/publish.py earth
```

### Assets

```bash
make ingest-asset
# -> ./out/asset/<sha256>.<ext>, and regenerates spec/spec/asset/ingested.json and
#    what the spec generates from it (the host's table, the catalog, the pack's layout)

for t in glb:model/gltf-binary png:image/png webp:image/webp; do
  AWS_REQUEST_CHECKSUM_CALCULATION=when_required aws s3 sync ./out/asset s3://pixelet-tiles/asset \
    --profile r2 \
    --exclude "*" --include "*.${t%%:*}" \
    --content-type ${t#*:} --cache-control "public, max-age=31536000, immutable"
done
```

No `Content-Encoding`. **Commit both regenerated tables with the asset change,
and upload before deploying.**

## Verify what is stored

```bash
# a tile: expect the manifest's ContentEncoding;
# a glb: expect ContentType model/gltf-binary and no ContentEncoding
aws s3api head-object --bucket pixelet-tiles --key terrain/<version>/<name> --profile r2
aws s3api head-object --bucket pixelet-tiles --key asset/<sha256>.<ext> --profile r2
```

## Verify what is served

After a publish, run the smoke:

```bash
./script/smoke.py https://plei.me
```

It fails when:

- a tile is compressed twice: the Worker's `Response` lost
  `encodeBody: "manual"` (`serve`);
- a second request, served from the edge cache, differs from the first
  (`CACHED_ENCODING`);
- an unbaked page returns anything but an empty 204 (HTML means
  `run_worker_first` is missing);
- an asset has the wrong type or a `content-encoding`.

It only warns when the second request missed the cache, which leaves that path
unchecked.

## Purging the edge cache

Only after fixing a serving bug whose wrong bytes are already cached.

```bash
# everything on the zone
curl -X POST "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/purge_cache" \
  -H "Authorization: Bearer $CF_API_TOKEN" -H "Content-Type: application/json" \
  --data '{"purge_everything":true}'

# or by URL, up to 30 per call
curl -X POST "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/purge_cache" \
  -H "Authorization: Bearer $CF_API_TOKEN" -H "Content-Type: application/json" \
  --data '{"files":["https://plei.me/tile/place/<version>/<name>"]}'
```

The token needs `Zone → Cache Purge`. Not `cache.delete(request)` in the
Worker: it clears one data center.
