# Bridges and tunnels as Arnis solves them

**2026-10-06** — the heights of bridges and tunnels are Arnis's, taken whole,
and every earlier rule for them is withdrawn.

Our world and Arnis's are one look: blocks, from OpenStreetMap's tags. A way
tagged alike should stand alike, so a structure's height is computed as Arnis
computes it (Apache-2.0; `src/element_processing/bridges.rs`, `highways.rs`,
`railways.rs`), less the blocks it places — the bake is headless. This replaces
the solve of [ways off the ground](./archive/20261001-ways-off-the-ground.md),
which reinterpreted Arnis rather than took it. That record's storage — a deck is
a line in the terrain page — stands. A contract of our own for the same surfaces
was drafted and aborted the day before
([archived](./archive/20261005-the-surface-a-way-runs-on.md)).

## What is taken

- **A road bridge** is one structure with every bridge it shares an end with at
  its layer and every deck running beside it. Its deck is the upper hull of what
  it must meet or clear: the terrain under it, a way at grade or a waterway it
  crosses plus that one's headroom, a lower deck plus the stacked headroom, each
  capped so the deck can still come down to the ends that rest on the ground; a
  long flat span rises to its layer; an end above the ground ramps down inside
  the deck, and a raised way at a free end becomes a ramp to the ground at its
  far end. Lower structures are solved first.
- **A rail bridge** rides a road deck of its layer it runs along, or is one
  level a viaduct with ramps at its free ends.

  > Since 2026-10-25, Arnis being the reference and the 1-ring the bound: a
  > structure is no longer solved as one line over all of it, which a ring that
  > sees part of it cannot do, and neighbouring tiles stepped where their rings
  > saw different parts. Whether decks stand side by side, a deck's axis and its
  > course are judged on the ways whole. A deck is solved in blocks of 16 cells
  > along its way, each from what lies about the block
  > (`pxl-geo-bake separation/solve/bridge.rs` `DECK_PANEL`, `DECK_WINDOW`,
  > `DECK_NEAR`), and a cell between two blocks is their blend. The ground
  > floors a deck and is no point of its line; a deck with no end of its own in
  > sight holds the highest it meets in sight, and an end at the ground holds
  > only its own way's deck. A track rides a deck where nearly all of it within
  > a window does. A long span with nothing high in sight sags to what is: over
  > the river at Seongsu `6023e5fa…` runs 24, 15, 20 where Arnis held it at 22
  > to 25. At Namsan, Bukak and three Han River bridges no deck crossing of a
  > seam differs.
- **A road tunnel** is as deep as its cover asks, and the ways at grade leading
  to a portal sink into it, cutting the ground (a grade line), so the bore
  starts at depth.

  > Since 2026-10-25: an approach is a grade line only where it sinks, to the
  > cell where it is back at grade; past that it is a way at grade, which the
  > ground paints.
- **A rail tunnel** runs at a fixed depth under the terrain; a subway is a
  tunnel wherever it is no bridge.
- **`layer`** is the source's `level_rules`, for height and order alike, as
  Arnis uses it.
- **A pier** is a beam bridge's, the style Arnis gives a bridge the source names
  no structure for: a bent every 8 m, 12 m past a half width of two cells, of
  one-cell columns at the middle, the edges or both; none where the deck is 2 m
  or less over the ground, on a way at grade or over a deck 5 m or more below.
- **What is built** is what Arnis builds: no bridge indoors or along a tunnel,
  no road tunnel indoors; a way indoors is nothing a deck clears.

What lies across a way, a structure's or what it clears, is the ground's: a
deck, a bore, a pier's bent and a way or a waterway under a bridge are as wide
as Carto draws them (`overture/width.rs`), where Arnis sizes each by its own
widths or clears a waterway's centerline. Its distances along and between ways
stay its own.

Arnis's numbers are metres — a block is a metre upright on the Earth (its
`celestial.rs`) — and each is taken into the page's cells by the page's scale,
`u_per_m` at the page's south-west corner (`pxl-spec procedural.rs`
`Frame::per_m`), rounded half to even (`pxl-spec-core` `cells_of_m`;
`pxl-geo-bake separation/solve.rs`). A deck's slab is Arnis's structure depth so
taken (`pxl-spec-core ways.rs` `occupies`, `slab_depth`), and a bore's clear
space is carried by its line (`pxl-spec bake/separation.rs` `clear`): its
ceiling under cover, the whole height to the sky where the roof is open.

## What is not taken

Arnis groups bridges by `bridge:name`, chooses deck modules and styles (arch,
suspension) and stops a bore at bedrock. The source carries no bridge name, the
styles are blocks, and the world has no bedrock, so none of these is ported.

What Overture says in Arnis's tags is this bake's reading. An Overture segment
carries its flags, its level and its subclass over ranges of itself, where an
OSM way holds its tags whole, so each segment is read as the ways its ranges cut
it into (`pxl-geo-bake segment.rs`): a bridge range is a way of its own, a cut
is an end its two parts share, and every rule here is Arnis's per way. `layer`
is `level_rules`; a ramp candidate is a way that is no bridge or tunnel, a road,
not indoors, at a level of 1 or more; a waterway to clear is a line of
Overture's water classed or subtyped river, canal or stream. A way not in use,
abandoned, disused or under construction, is none, though Arnis builds some of
them: the world draws nothing that is not in use.

## What it leaves

- **A tunnel that opens on a bridge's middle** stays at the terrain while the
  deck passes over it. At Namsan (127.00451 37.55363) the footpath tunnel
  `1821b44d…` starts at connector `68feb3a4…`, which sits at 0.569 of the bridge
  segment `ba443603…`: the bridge has no end there to come down to, and a
  portal's approach is only a way at grade, so the bore's mouth is at 44 under a
  deck at 50. Arnis leaves it so, and so does this.

  > Since fixed (2026-10-25): a road tunnel's portal on a road deck, at the
  > deck's end or along it, opens at the deck's height and ramps down from it
  > with no approach to sink, and where that holds it over the ground it is an
  > embankment up to it; between two decks it runs straight from one to the
  > other. `1821b44d…` lies between two decks, `ba443603…` at 52 and `698a1efb…`
  > at 45, and runs from 51 to 45: a step of a cell a cell leaves it one short
  > of the higher deck.
- **A tunnel the ring cuts** keeps its cut ends at depth, so two pages whose
  rings cut it in different places solve it differently: one cell of one seam of
  nine pages at Namsan.

  > Since fixed (2026-10-25). The pages did not differ for want of seeing: two
  > neighbours both see the cells where they meet. They differed because the
  > bore's floor was drawn from where each ring cut it: the straight run between
  > its two ends, a cut one among them, and its one valley, the lowest point of
  > the stretch it saw. A cut lies a tile from every cell its page draws, so a
  > floor that leans on nothing further than a tile is the same in both. A bore
  > no longer than a tile stays Arnis's, since every page that draws any of it
  > holds all of it. A longer one's floor is the highest under its cover by its
  > drop that rises at most 2% from the lowest such floor within a tile, raised
  > to whichever portal its page sees, with no one valley: the straight run
  > between its portals is not a ring's to know, and is not looked for past it,
  > which would cost every page reads it is not owed
  > (`pxl-geo-bake separation/solve.rs`, the module's rules). It stands up to 2%
  > above that straight run where it is furthest from low cover: `c739a673…`
  > runs from 39 up to 42 and down to 27 where Arnis runs from 39 to 28. A road
  > tunnel whose end lies along another stays at depth there, as at an end the
  > two share. Measured at the seams, where a line crosses the edge two pages
  > share: at Namsan 0 of 49 tunnel crossings differ (4 did, by up to 4 cells),
  > and at Bukak (126.9845 37.6025) 0 of 6 (4 did, by up to 14).

It is the page bake's solve (`pxl-geo-bake separation/solve.rs`); the earlier
one and the structures it kept between pages are gone.

Measured at Namsan, nine pages each solved from its own ring: 0.08 to 0.41 s a
page, of which the solve is 0.01 to 0.04 s and the rest reading; of twelve
seams, eleven draw every shared cell within a step and one draws one cell two
apart.
