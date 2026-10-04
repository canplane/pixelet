# A snapshot of a subtree

**2026-10-04** — a design, decided and not built.

A published bake is an immutable snapshot of a whole kind: terrain, POI or
geocode, named `v<format>.<day>.<n>` and never rewritten (bake-tiles,
"Versions"). Some changes will touch one region: a fix to how a district's
bridges are baked, a refreshed city. Baking and uploading the whole kind for
them costs a pyramid's worth of objects. This is how a region is baked and
published alone without giving up the snapshot: a **partial snapshot**, a
copy-on-write snapshot of the pyramid that replaces one or more subtrees of
a full one.

## A region is a quadkey prefix

The bucket names a page by its quadkey, and a prefix is one region at every
level and in every kind (publish-bucket, "What the bucket holds"). So a
region needs no unit of its own: it is a prefix, and a partial snapshot
owns the pages under each of its prefixes.

## What a partial snapshot holds

The pages under its regions, baked again, and every ancestor of a region up
to the pyramid's top, folded again from the new pages and from their
siblings in the base. Nothing else. The levels agree with each other because
every page whose content depends on a replaced page is itself replaced.

Its manifest says two things beyond what any bake's does:

```json
{ "base": "v7.2026-10-04.1", "regions": ["132110", "132123"] }
```

The ancestors are not listed; they follow from the regions.

## Which folder answers a page

A page belongs to the partial snapshot when its quadkey starts with one of
the regions or one of the regions starts with its quadkey. Such a page is
read from the partial snapshot's folder and from nowhere else: **a page it
owns and does not hold is no object**, a 404, never a fall through to the
base. A region baked again can lose a page — a POI page pruned because what
it counted left — and a fall through would bring the old one back. Every
other page is the base's.

The rule asks the manifest which pages are owned, never whether an object
exists.

## Where the rule runs

In the Worker: the client asks for one version name, as it does now, and the
Worker maps a partial snapshot's name to `{ base, regions }` and the request
to one of the two folders. That is the place a canary would choose a folder
per request too, so the table is the same table.

The dev tile server's `--baked` answers a partial snapshot by the same rule,
so it is looked at before it is uploaded.

## What is ruled out

- **A chain.** A partial snapshot's base is always a full bake, never
  another partial one, so a page is found in one step and no stack is
  resolved. Two regions changed together are one partial snapshot with two
  prefixes; a third later is a new partial snapshot over the same full base
  with all three, or a new full bake.
- **Rewriting a folder.** Every folder stays immutable, so going back is
  naming the version before, as for a full bake.
- **A finer unit than a page.** A page is still the smallest thing baked
  and uploaded.

## Not built

Nothing of this exists yet: the bake tools take no region, the manifest no
base, the Worker no table. It is built when a region first needs baking
alone.
