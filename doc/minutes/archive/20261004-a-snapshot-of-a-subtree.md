# A snapshot of a subtree

**2026-10-04** — decided and built the same day.

A published bake is an immutable snapshot of a whole kind: terrain, POI or
geocode, named `v<format>.<day>.<n>` and never rewritten (bake-tiles,
"Versions"). Some changes touch one region: a fix to how a district's
bridges are baked, a refreshed city. Baking and uploading the whole kind for
them costs hours and a pyramid's worth of objects. A **partial snapshot** is
a new version that holds only what changed over one full version — a
copy-on-write snapshot of the pyramid.

## What changes, and nothing more

A region is a quadkey prefix: the bucket names a page by its quadkey, and a
prefix is one region at every zoom. `bake_tiles --region` bakes the land
pages under it again and folds again, a zoom at a time, only the parents whose
`Folds::sources` read a page whose content changed, until a zoom changes
nothing. Every page it writes is what a full bake of the same code writes —
checked against a full assemble over the same leaves, 226,623 pages, none
differing.

What changed is not the region and its ancestors. A terrain parent reads the
ring around its children for its elevation's padding, so the parents beside
an ancestor change too: six at z15 in that check, where the ancestors
alone are one. So a partial snapshot's pages are **listed**, never derived
from its regions.

## The list

Beside its pages, a partial version keeps `_snapshot`:

```json
{ "base": "v7.2026-10-04.1", "written": ["132110320312000", "…"], "removed": [] }
```

`written` is every page whose bytes differ from the base's, `removed` every
page the base has and it does not. The bucket holds only the written pages
and the list.

The base is always a full version. A region baked over a partial version
lists its changes over that one's base, so its own pages carry the earlier
partial's too, bytes and all: a version is one full version and one list,
and is found in one step, never through a chain.

## Which object answers a page

The Worker reads the list before both GET and HEAD:

| The page is               | It is read from       |
| ------------------------- | --------------------- |
| in `written`              | the partial version   |
| in `removed`              | nowhere: 404          |
| anything else             | the base              |
| of a version with no list | that version, as ever |

A page the partial version owns and does not hold is a 404, never a fall
through to the base, which would bring back a page the region's bake let
go. The list in the bucket is what decides. The Worker keeps a list it read,
since a version never changes what it lists, but not that a version had none,
so a list that arrives last still counts: it is uploaded after the pages it
names. A cache miss on a full version costs one more read, for the list that
is not there — bounded by the Workers plan's daily requests, inside R2's free
reads. Publishing
a partial version is a client change, as any version is, and no deploy of
the Worker.

## Where it can go wrong

- The region must hold everything the change reaches. A structure solved
  again that runs out of it meets the base's solve of it at the region's
  edge; choosing the region is the caller's job.
- A full version a published partial names is not deleted from the bucket:
  every page the partial does not list is read from it.
