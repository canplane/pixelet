# A place name is a rank and an id

**2026-09-23** — measured against the bake in `out/geocode`, built, not
deployed. Reverse geocoding answers a page with the names of the places that
contain it. There are a few thousand such places in the country and 167,495
pages, so what the bake wrote was one small dictionary, spelled out over and
over — 3,426 distinct answers across 167,495 objects.

Two things follow from that one number, and both are now built. A part of an
answer becomes a **rank** and an **id** into a dictionary fetched once, so the
names are stored once. And the page stops being the unit: neighbouring pages
give the same answer, so an object covers a square of 256 of them. The set is
**902 objects and 1.29 MB** where it was 167,495 objects and 30.1 MB.

## What a page held

A page is the same L3 page terrain's leaves are, 512 cells of Web Mercator's
z16 — 1,223 m a side, which is exactly one z15 tile. `bake_geocode_tiles`
walked the 167,499 land pages, asked Overture's Divisions for the label at
each page's centre, and wrote one object for each of the 167,495 that were in
any place at all:

```json
{
  "parts": [
    { "name": "경상남도", "common": { "en": "South Gyeongsang", "ko": "경상남도" } },
    { "name": "하동군", "common": { "en": "Hadong-gun", "ko": "하동군" } },
    { "name": "중평리", "common": { "en": "Jungpyeong-ri", "ko": "중평리" }, "approx": true }
  ]
}
```

`parts` runs broad to narrow. Which parts there are is the provider's ladder:
every `region` and `county` polygon containing the centre, then the first
match among `locality`, `neighborhood`, `microhood`, `macrohood`, and where
no polygon gives a fine one, the nearest division point within two
kilometres — `approx` marks that one as an approximation. Joining the parts
into a line, and picking which language's name to show, is the reader's,
because broad-to-narrow is a language convention rather than a property of
the place.

Measured over an even sample of 4,000 of the 167,495 objects:

|                         |                                 |
| ----------------------- | ------------------------------- |
| one page, raw JSON      | 205 B                           |
| one page, stored        | 180 B, gzip                     |
| the set                 | 30.1 MB                         |
| parts a page            | 2 for 20%, 3 for 38%, 4 for 42% |
| **distinct names**      | **3,702**                       |
| **distinct part lists** | **3,426**                       |

Three and a half thousand distinct answers, written 167,495 times.

## What a reader asks for

[Nominatim](https://nominatim.org/release-docs/latest/api/Reverse/) takes a
`zoom` on its reverse endpoint and answers at that depth:

| zoom | 3       | 5     | 8      | 10   | 12   | 13      | 14            | 15         | 16            | 17        | 18       |
| ---- | ------- | ----- | ------ | ---- | ---- | ------- | ------------- | ---------- | ------------- | --------- | -------- |
|      | country | state | county | city | town | village | neighbourhood | settlement | major streets | and minor | building |

What makes that a lookup rather than a search is that every part of an
address carries a rank — `address_rank` in Nominatim's own tables, `types`
in Google's response, `region.area1..4` in Naver's. A reader asks for a
depth and the parts past it are dropped.

The page above carried no rank. Its parts were in order, but nothing said
what any of them was, so a client could not cut the list at a zoom without
knowing the provider's ladder — which is exactly what the wire type was
written not to expose.

## A part is a rank and an id

**The rank is ours, not a provider's.** A small fixed ladder — country,
region, county, city, district, neighbourhood, street, building — and every
provider maps its own vocabulary onto it: Overture's `region`/`county`/
`locality`, Naver's `area1`..`area4`, Google's `administrative_area_level_*`
and `sublocality_level_*`. The provider owns the mapping, the wire owns the
ladder. That is the line `geocode/result.rs` claimed to draw and did not:
`common` was Overture's `names.common` blob passed through unread, so a
provider with a different name structure could not fill it.

The **dictionary** is the other half. It holds every place a polygon names,
once — its rank, its own name, and that name by language — so an answer names
the places it is made of by number and spells none of them.

```
an answer   [ (rank, id), ... ]        three or four numbers
dictionary  id -> { rank, names by language }
```

It is one object fetched once and held: 5,266 divisions for the whole
country, 88 KB stored and 619 KB read where it lies. The ladder is spaced by
ten so a provider with a level between two rungs has a number to be given and
no existing rung moves to make room.

## A page is not the unit

3,426 distinct answers over 167,495 pages does not only mean the names
repeat. It means whole answers repeat, and they repeat _next to each other_:
a district is hundreds of pages wide and every page of it reads the same.

So the object is a **group** — the page four levels up, 16 leaves on a side,
256 of them — holding the answers its leaves take and one index a leaf.

```
group   answers: [ (rank, id), ... ] each   what the pages under it are in
        page[256] -> an answer
```

The same 167,499 land pages fall into **902 groups**, 186 pages a group, and
54% of the groups are full at 256. One of them is in no place at all and has
no object; the other 901 hold 37,145 answers between them, 41 a group, in a
median 1,092 B and at most 4,396. A reader crossing a district fetches once
rather than once a page, and the district's name is stored once for the group
rather than once for each of its pages.

One thing does not fit in the dictionary. A part that came from a place
_near_ a page rather than one containing it comes from the provider's
points — a different table, forty times the size, almost none of it ever
named. A group carries the few of those it needs itself, and the bit that
marks a part as an approximation is the bit that says which table to read it
from, so one bit does both.

## Where this sits among the ways of doing it

[SFO Museum's point-in-polygon service](https://millsfield.sfomuseum.org/blog/2022/12/19/pmtiles-pip/)
is the same shape as this deployment: no spatial database at request time,
a static file in a bucket, a range request for the one tile a coordinate
falls in. It stores **geometry** — Who's On First polygons as vector tiles —
and runs point-in-polygon in memory per query, for about a dollar a month.

This bake stores the **answer** instead, resolved offline per page. That is
the faster of the two at request time by a whole algorithm: there is no
polygon test, only an object to fetch. What it costs is that the depth is
decided when the page is baked — which is what the rank buys back, since the
reader now does the cutting.

## What it came to

|                 | before        | after                           |
| --------------- | ------------- | ------------------------------- |
| objects         | 167,495       | 902                             |
| the set, stored | 30.1 MB, gzip | 1.29 MB, zstd                   |
| one object      | 180 B a page  | 1,092 B a group, median         |
| the dictionary  | —             | one object, 88 KB, 5,266 places |

The reader is the kernel, not the shell. The page format is `pxl-spec`'s, so
what opens it is where `pxl-spec` compiles: the kernel holds the dictionary
and the group the map centre is in, reads a name out of the packet where it
lies, cuts the parts at the rung the current zoom asks for, and hands the UI
snapshot the parts unjoined and in every language the provider had. Picking a
language and joining stays the reader's, so switching UI language re-orders a
label already in hand instead of asking for another.

Overture's own vocabulary stayed where it belongs. `BROAD_SUBTYPE_ORDER` and
`FINE_SUBTYPE_PRIORITY` are still the provider's — the first says which of two
containing polygons is broader, the second which of four sub-city subtypes
wins where several contain one point — but neither leaves the provider now:
`rank_of` turns a subtype into a rung, and nothing past it knows the word
`locality`.

Three things went with it: the shell's own debounce, abort and
already-asked bookkeeping, which existed because the shell has no scheduler;
the three TypeScript tile-base builders, a second copy of what the kernel's
route module already did for terrain and POI; and `GeocodePart`'s `common`,
now `names` on both sides of the wire.

The useful zoom range is **z5 to z15**. Below z5 the ladder asks for a country
and Korea's divisions have none, so the label is empty; above z15 the answer
cannot get finer, because its granularity is one page. The `city` rung is
unused here — Overture files a 시 as a `county` — so z10 and z11 read exactly
as z8 and z9 do.

## Open

- POI has the same shape of question — one object a page, names spelled out —
  and is not in this note.
- A part below a neighbourhood needs a street or an address, which is a
  different source. Nothing here reaches z16.
