---
name: publish-bucket
description: Upload baked tiles, ingested models and textures to the R2 bucket the Worker serves, and verify the served bytes. Use after a bake or an asset change, when the client must point at a new version, or when a live tile decodes wrong.
---

# Publishing the bucket

How a tile set or an asset gets into the bucket `pixelet-tiles`, and how to
confirm it arrived. What the bucket holds and how the Worker serves it is
[infra/README.md](../../../infra/README.md) §4; building a set is
[bake-tiles](../bake-tiles/SKILL.md).

The client reads which versions to request from the bucket's `source` object at
boot, and only `./script/publish.py` writes it, so publishing needs no commit or
deploy. Object names have no extension, so **content-type and content-encoding
are stored with each object at upload**: a manual upload must set both.

## One-time setup

In the Cloudflare dashboard, create the bucket with the name in
`ts/server/edge/wrangler.jsonc` (`r2_buckets[0].bucket_name`) and keep it
**private**; create an R2 API token (an S3-compatible key ID and secret); note
the account ID. Then make a separate AWS profile so the default one is left
alone:

```bash
aws configure --profile r2          # region: auto
aws configure set endpoint_url https://<account_id>.r2.cloudflarestorage.com --profile r2
```

`publish.py` uses that profile (`$R2`, if set, overrides its endpoint). A manual
`aws s3 sync` also needs `AWS_REQUEST_CHECKSUM_CALCULATION=when_required`,
because R2 refuses the CLI's checksum headers; `publish.py` sets it itself.

## Upload

**Every object uploaded is one Class A operation, and nothing stops you at the
free million a month**: the plan's fail-closed limit covers reads, not writes.
A terrain version is every tile, so a few of them use up the month. Check usage
before and after, against the object count `publish` prints:

```bash
./.agents/skills/publish-bucket/usage.py     # Class A this month against the million
```

Deleting an old version is free.

```bash
make bake-<kind> ARGS="--stage publish"    # terrain, place or division
./script/publish.py <kind>
```

`publish` names the version, uploads with the manifest's content-encoding,
checks the stored count, then writes `source`.

```bash
./script/publish.py serve <kind> <version>              # serve an older version again: how to roll back
./script/publish.py override <shard> <kind> <version>   # one shard and the tiles under it; - drops it
```

**Never overwrite an existing version**, of any kind: publish a new one and
serve it.

A missing place or division tile is not an error: no reader asked for it, and
the client reads the tile above it.

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

One sync per type, so each object gets its content-type. No `Content-Encoding`:
glb, png and webp are already compressed. **Commit both regenerated tables with
the asset change, and upload before deploying**: a key the shipped table names
but the bucket lacks is a 404 on the live site.

## Verify what is stored

```bash
# a tile: expect the manifest's ContentEncoding;
# a glb: expect ContentType model/gltf-binary and no ContentEncoding
aws s3api head-object --bucket pixelet-tiles --key terrain/<version>/<name> --profile r2
aws s3api head-object --bucket pixelet-tiles --key asset/<sha256>.<ext> --profile r2
```

## Verify what is served

The bucket can be right while serving is wrong. CI runs the smoke after every
deploy; after a publish (which is not a deploy), run it yourself:

```bash
./script/smoke.py https://plei.me
```

It fails when:

- a tile is compressed twice: the Worker's `Response` lost
  `encodeBody: "manual"` (`serve`);
- a second request, served from the edge cache, differs from the first
  (`STORED_ENCODING`);
- an unbaked page returns anything but an empty 204 (HTML means
  `run_worker_first` is missing);
- an asset has the wrong type or a `content-encoding`.

It only warns when the second request missed the cache, which leaves that path
unchecked.

## Purging the edge cache

Only after fixing a serving bug whose wrong bytes are already cached; a version's
or an asset's URL never changes meaning.

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

The token needs `Zone → Cache Purge`. `cache.delete(request)` inside the Worker
is no substitute: it clears only the data center that ran it.
