# Bridges and tunnels as Arnis solves them

**2026-10-06** — the heights of bridges and tunnels are Arnis's, taken
whole, and every earlier rule for them is withdrawn.

Our world and Arnis's are one look: blocks, from OpenStreetMap's tags. A
way tagged alike should stand alike, so a structure's height is computed
as Arnis computes it (Apache-2.0; `src/element_processing/bridges.rs`,
`highways.rs`, `railways.rs`), less the blocks it places — the bake is
headless. This replaces the solve of
[ways off the ground](./20261001-ways-off-the-ground.md), which
reinterpreted Arnis rather than took it. That record's storage — a deck is
a line in the terrain page — stands. A contract of our own for the same
surfaces was drafted and aborted the day before
([closed](./closed/20261005-the-surface-a-way-runs-on.md)).

## What is taken

- **A road bridge** is one structure with every bridge it shares an end
  with at its layer and every deck running beside it. Its deck is the
  upper hull of what it must meet or clear: the terrain under it, a way at
  grade or a waterway it crosses plus that one's headroom, a lower deck
  plus the stacked headroom, each capped so the deck can still come down to
  the ends that rest on the ground; a long flat span rises to its layer;
  an end above the ground ramps down inside the deck, and a raised way at
  a free end becomes a ramp to the ground at its far end. Lower structures
  are solved first.
- **A rail bridge** rides a road deck of its layer it runs along, or is one
  level a viaduct with ramps at its free ends.
- **A road tunnel** is as deep as its cover asks, and the ways at grade
  leading to a portal sink into it, cutting the ground (a grade line), so
  the bore starts at depth.
- **A rail tunnel** runs at a fixed depth under the terrain; a subway is a
  tunnel wherever it is no bridge.
- **`layer`** is the source's `level_rules`, for height and order alike, as
  Arnis uses it.
- **A pier** is a beam bridge's, the style Arnis gives a bridge the source
  names no structure for: a bent every 8 m, 12 m past a half width of two
  cells, of one-cell columns at the middle, the edges or both; none where
  the deck is 2 m or less over the ground, on a way at grade or over a deck
  5 m or more below.
- **What is built** is what Arnis builds: no bridge indoors or along a
  tunnel, no road tunnel indoors; a way indoors is nothing a deck clears.

What lies across a way, a structure's or what it clears, is the ground's:
a deck, a bore, a pier's bent and a way or a waterway under a bridge are as
wide as Carto draws them (`overture/width.rs`), where Arnis sizes each by
its own widths or clears a waterway's centerline. Its distances along and
between ways stay its own.

Arnis's numbers are metres — a block is a metre upright on the Earth (its
`celestial.rs`) — and each is taken into the page's cells by the page's
scale, `u_per_m` at the page's base, rounded half to even
(`pxl-geo-bake deck/arnis.rs`). A deck's slab is Arnis's structure depth so taken
(`pxl-spec bake/deck.rs` `occupies`), and a bore's clear space is carried by
its line (`clear`): its ceiling under cover, the whole height to the sky
where the roof is open.

## What is not taken

Arnis groups bridges by `bridge:name`, chooses deck modules and styles
(arch, suspension) and stops a bore at bedrock. The source carries no
bridge name, the styles are blocks, and the world has no bedrock, so none
of these is ported.

What Overture says in Arnis's tags is this bake's reading. An Overture
segment carries its flags, its level and its subclass over ranges of
itself, where an OSM way holds its tags whole, so each segment is read as
the ways its ranges cut it into (`pxl-geo-bake segment.rs`): a bridge
range is a way of its own, a cut is an end its two parts share, and every
rule here is Arnis's per way. `layer` is `level_rules`; a ramp candidate is
a way that is no bridge or tunnel, a road, not indoors, at a level of 1 or
more; a waterway to clear is a line of Overture's water classed or
subtyped river, canal or stream. A way not in use, abandoned, disused or
under construction, is none, though Arnis builds some of them: the world
draws nothing that is not in use.

## What it leaves

- **A tunnel that opens on a bridge's middle** stays at the terrain while
  the deck passes over it. At Namsan (127.00451 37.55363) the footpath
  tunnel `1821b44d…` starts at connector `68feb3a4…`, which sits at 0.569
  of the bridge segment `ba443603…`: the bridge has no end there to come
  down to, and a portal's approach is only a way at grade, so the bore's
  mouth is at 44 under a deck at 50. Arnis leaves it so, and so does this.
- **A tunnel the ring cuts** keeps its cut ends at depth, so two pages
  whose rings cut it in different places solve it differently: one cell of
  one seam of nine pages at Namsan.

It is the page bake's solve (`pxl-geo-bake deck/arnis.rs`); the earlier one
and the structures it kept between pages are gone.

Measured at Namsan, nine pages each solved from its own ring: 0.08 to 0.41 s a page, of which the solve is 0.01 to 0.04 s
and the rest reading; of twelve seams, eleven draw every shared cell
within a step and one draws one cell two apart.
