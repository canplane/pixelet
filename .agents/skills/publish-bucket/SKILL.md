---
name: publish-bucket
description: Upload baked tiles, ingested models and textures to the R2 bucket the Worker serves, and verify the served bytes. Use after a bake or an asset change, when the client must point at a new version, or when a live tile decodes wrong. Building a set is the bake-tiles skill.
---

# Publishing the bucket

The bucket is `pixelet-bucket` ([infra/README.md](../../../infra/README.md) §4).

Only `./script/publish.py` writes the bucket's `source` object, which the
client reads at boot: publishing needs no commit or deploy. **A manual upload
sets content-type and content-encoding.**

`publish.py` reaches the bucket through `script/r2.py`, with the key
[infra/README.md](../../../infra/README.md) §4 sets up.

## Upload

**Each uploaded object is one Class A operation, and nothing stops writes at
the free million a month**; a terrain version is every tile. Check usage before
and after, against the object count `publish` prints:

```bash
./.agents/skills/publish-bucket/usage.py     # Class A this month against the million
```

Deleting an old version is free.

```bash
make bake-<kind> ARGS="--step publish"     # terrain, place or division
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

A client takes `source` only where every kind is in its own format exactly
(pxl-core `sys/io/source.rs`, `parse`). A version in a new format goes up with
`upload` and is served with `serve` once the client that reads it is deployed.

### Earth

The earth's bake writes its own `out/earth.publish/`
([bake-tiles](../bake-tiles/SKILL.md)); `./script/publish.py earth` uploads it.

### Assets

Models and textures are ingested and synced as
[asset/README.md](../../../asset/README.md) says, before the deploy that reads
them.

## Verify what is stored

```bash
# a tile: expect the manifest's ContentEncoding;
# a glb: expect ContentType model/gltf-binary and no ContentEncoding
aws s3api head-object --bucket pixelet-bucket --key terrain/<version>/<name> --profile r2
aws s3api head-object --bucket pixelet-bucket --key asset/<sha256>.<ext> --profile r2
```

## Verify what is served

After a publish, run `./script/smoke.py`. What a failure means is the deploy
skill's ([deploy](../deploy/SKILL.md), "When the smoke fails").

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
  --data '{"files":["<site>/tile/place/<version>/<name>"]}'   # the site: route.json5 `origins`
```

The token needs `Zone → Cache Purge`. Not `cache.delete(request)` in the
Worker: it clears one data center.
