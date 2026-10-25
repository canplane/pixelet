---
name: publish-bucket
description: Upload baked tiles, ingested models and textures to the R2 bucket the Worker serves, and verify the served bytes. Use after a bake or an asset change, when the client must point at a new version, or when a live tile decodes wrong.
---

# Publishing the bucket

How a set or an asset gets into the bucket `pixelet-tiles`, and how to know it
arrived. What the bucket holds, and how the Worker serves it, is
[infra/README.md](../../../infra/README.md) §4; producing a set is
[bake-tiles](../bake-tiles/SKILL.md).

The versions the client requests are the bucket's `source`, which
`./ts/app/web/script/publish.ts` alone writes and the client reads at boot, so a publish is
never a commit or a deploy. An object's name carries no extension, so **its
content-type and content-encoding are stored with it at upload**: a manual
upload must set both.

## One-time setup

In the Cloudflare dashboard: create the bucket, named as `wrangler.jsonc`'s
`r2_buckets[0].bucket_name` and kept **private**; create an R2 API token (an
S3-compatible key ID and secret); note the account ID. Then, in a profile of
its own so a default AWS profile is left alone:

```bash
aws configure --profile r2          # region: auto
export R2=https://<account_id>.r2.cloudflarestorage.com
```

`publish.ts` uses exactly that profile and `$R2`. A manual `aws s3 sync` needs
`AWS_REQUEST_CHECKSUM_CALCULATION=when_required` too, since the CLI's checksum
headers are refused by R2; `publish.ts` sets it for itself.

## Upload

**A publish costs one Class A operation an object**, and nothing stops at the
free month's million: the plan's fail-closed limit covers reads, not writes. A
terrain version is every tile, so a few in a month is past it. Before an upload
and after:

```bash
./script/usage.ts     # Class A this month against the million
```

and set the object count `publish` prints against what is left. Deleting an
old version is free.

```bash
make bake-<kind> ARGS="--stage publish"    # terrain, place or division
./ts/app/web/script/publish.ts <kind>
```

`publish` names the version, uploads the copy with the manifest's
content-encoding, checks the stored count, then writes `source`.
`./ts/app/web/script/publish.ts serve <kind> <version>` serves a version the bucket holds
again, which is how to go back; `./ts/app/web/script/publish.ts override <shard> <kind>
<version>` serves a version for one shard and the tiles under it, `-` dropping
it.

**Never overwrite an existing version in place**, of any kind: publish a new
one and serve it.

For places and divisions, a tile no reader asks for is no object, and the
client reads its ground from the tile over it, so a missing tile is never an
error to repair.

### Earth

```bash
make download-earth
make bake-earth
./ts/app/web/script/publish.ts earth
```

### Assets

```bash
make ingest-asset
# -> ./out/asset/<sha256>.<ext>, and regenerates spec/spec/asset/ingested.json and
#    what the spec generates from it (the host's table, the catalog, the pack's layout)

for t in glb:model/gltf-binary png:image/png webp:image/webp; do
  AWS_REQUEST_CHECKSUM_CALCULATION=when_required aws s3 sync ./out/asset s3://pixelet-tiles/asset \
    --profile r2 --endpoint-url $R2 \
    --exclude "*" --include "*.${t%%:*}" \
    --content-type ${t#*:} --cache-control "public, max-age=31536000, immutable"
done
```

One sync per type, so each object keeps its content-type; no `Content-Encoding`,
since glb, png and webp are already compressed. **Commit both regenerated tables
with the asset change, and upload before deploying**: a key the shipped table
names that is not in the bucket is a 404 on the live site.

## Verify what is stored

```bash
# expect the manifest's ContentEncoding on a tile;
# ContentType model/gltf-binary and no ContentEncoding on a glb
aws s3api head-object --bucket pixelet-tiles --key terrain/<version>/<name> --profile r2 --endpoint-url $R2
aws s3api head-object --bucket pixelet-tiles --key asset/<sha256>.<ext> --profile r2 --endpoint-url $R2
```

## Verify what is served

The bucket can be right while the serving path is not. The smoke checks the
served bytes, and CI runs it after every deploy; run it by hand after a
publish, which is not a deploy:

```bash
./ts/app/web/script/smoke.ts https://plei.me
```

It fails on a tile packed twice (the Worker's `Response` lost
`encodeBody: "manual"`, `serve`), on a second ask off the edge cache that
differs from the first (`STORED_ENCODING`), on an unbaked page answered with
anything but an empty 204 (HTML means `run_worker_first` is missing), and on
an asset with the wrong type or a `content-encoding`. It only warns when the
second ask did not hit the cache, which leaves that path unchecked.

## Purging the edge cache

Only after fixing a serving bug whose wrong bytes are already cached: a URL of a
version or an asset never changes meaning.

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
is no substitute: it clears only the colo that ran it.
