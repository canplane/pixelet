# The screen keeps its tiles as the light does

**2026-10-29, revised 2026-10-30.** How a frame that is not drawn whole
decides what to draw again. Begun as a draft that copied the light's cast
onto the screen, it was reviewed twice between two sessions against main at
5553036df, and taken by the user, who asked for the screen and the light to
be structurally symmetric: the screen deciding what to draw again by the
shape the light's cast already decides what to cast by. Revised after a
review against the performance skill (`.agents/skills/performance/SKILL.md`).

> One shape for both: a word a tile, marked, reached across, claimed, and
> drawn where claimed. The light's reach takes the screen's receivers to the
> light's tiles; the screen's takes the light's changed tiles back to the
> pixels that read them. Each claims by its own role: the light a tile not
> yet cast in its epoch, the screen a tile whose history has not yet
> gathered.

## Adopted and proposed

What the user decided, each by their word, and so binds (doc/README.md):

- **The screen and the light structurally symmetric**: the screen deciding
  what to draw again by the shape the light's cast decides what to cast by,
  one mark → reach → claim (2026-10-29; held again 2026-10-30).
- **One set of minutes**, this one (2026-10-30).
- **The other session's line first**, these steps on top of it
  (2026-10-30).
- **The steps kept**: steps 3 and 5 taken for the symmetry, not dropped
  because the picture is right without them (2026-10-30).
- **Steps 3 → 5 → 6**, step 4 being done (2026-10-30).
- **No patch to the depth's bounds** at a render scale other than 1; step 5
  is the fix (2026-10-30). Narrowed by the user the same day once the cause
  was found to be a lifetime bug, not an approximation: **the read-back's
  lifetime fixed on main** (`c9e822e4a`); the path itself still goes in
  step 5.
- **The structural, symmetric path only**: no ad-hoc fix, and no local
  optimization that weakens a symmetry; a change to a symmetry or a boundary
  proposed on its own as a design change (2026-10-30).
- **The screen's tile words the GPU's, as the light's are**, from step 5:
  the reach marks screen tiles on the GPU, so the CPU's words handed each
  frame would undo its marks; the CPU hands its marks and decides whole,
  gated or stood, and the GPU keeps the words (2026-10-30).
- **Names**, chosen with the user (2026-10-30): the frame kind `gated` (was
  `region`), in the whole / gated / stood of Proposed 5; `resolve::Mark` and
  `mark_of`, what a draw that changed marks, the light's word
  (`light_mark_cs`); a screen tile's word `changed_at`, the frame it last
  changed in, as a light tile's names a cast (`reads_at`); the raster's gate
  `raster_gate`, as the world's is `screen_gate`. For step 5: `Tiles`
  (gpu/tiles.rs), the words, their uniform and their marks' pass, one for
  the screen and one for the light's map, as the shaders' uniform is named;
  `claimed_for`, the uniform's word for how many frames a tile stays claimed
  once changed (the screen `JITTERS`, the map 1, every one where the frame
  is drawn whole), in place of the flag for whole; the map's gate
  `light_gate`; the map's tiles 32 texels a side, the screen's `TILE`, since
  narrowed by the user to 8 texels, the light's cast's tile (`light::TILE`),
  once the reach was measured to claim the whole footprint of a 32-texel
  tile a caster touched (2026-10-30); each instance's side in its uniform.
  Confirmed with the user the same day: the passes `screen_mark_cs` and
  `screen_reach_cs`, as the light's `light_mark_cs` and `light_reach_cs`;
  the span `mark`; `Reach` (gpu/reach.rs) and its word `reached`;
  `Gpu::gated_depth`, the world's depth and the map's.
- **The host reads back whether the reach marked**, one word, the number of
  the last frame the reach marked a screen tile in, read back as the
  light's counts are; until it lands the tiles are taken to gather, after
  it exactly (2026-10-30). Step 5 answers here what was left to step 6 for
  the reach's marks, which the host cannot see. (Asked on a misreading,
  since corrected in the baseline's record: the frames that never stood
  did so on part 2 too, the companion's draws changing each turn.)
- **The reach reads a depth's bound kept on the GPU**, a square of 4 pixels
  a side its least and most depth, set again in the tiles each frame draws,
  in place of every pixel's depth read each frame it runs (2026-10-30):
  that reading was measured to be nearly all of its cost.

Everything else below is proposed, reviewed between two sessions and taken
in at the user's word to go on with, but not decided by them: the tile's
size and word, claim by gathering, the CPU's part, the gate and its
numbered stencil, the passes over the whole screen as the tiles' quads, the
state kept across frames and its conditions, the fallback without a
stencil, the invariants' axes and baseline, and the Not taken.

## What stood on main at 5553036df

Superseded in part by `e6b740ab8`, as marked; kept for why the steps are
what they are. Do not solve again what is marked fixed.

A frame where only what the kernel animates moved draws one rectangle again
(pxl-render gpu/renderer.rs `regions`, gpu/surface.rs `Region`), bounded on
the CPU by the kernel's spheres (pxl-kernel domain/scene/region.rs). Its cost
follows its size. Measured at the view the page opens at, noon, shadows on,
scale 0.84 (agent run, 2026-10-29):

- a region frame: GPU p50 11.9 ms, the region about 42% of the screen;
- GPU time ≈ 3.0 ms fixed + 0.21 ms per percent of the screen;
- shadows off: 5.4 ms; zoomed out (region ≈ 4%): 3.9 ms;
- at 7:00, region frames too: 15.1 ms at scale 1.0.

Why the region was that large:

- An actor's sphere was as far as its clips reach, the one it fades from too
  (pxl-kernel actor.rs `Actor::draw`), not its pose: the companion is 1 u
  tall and its sphere 2.07 u, its sprint's 6.38 u. **Fixed in `e6b740ab8`**
  (`Actor::pose_reach`).
- The puck's was the zone and the beacon together, its state the beacon's
  bob, so every bob moved the zone's 3.3–4.8 u footprint. **Fixed in
  `e6b740ab8`** (the zone and the beacon two).
- A caster's shadow was a capsule scaled by its radius (`SHADOW_RADII`).
  Replaced in `e6b740ab8` by a light-plane test against the depth's bounds
  read back, itself removed by step 5.
- All of it is one rectangle (`Region::of`). Removed by step 3.

Seen wrong on main:

- **No pixel had an age.** A frame drawn whole that is not quiet blends at
  no less than `BLEND_LEAST` (gpu/resolve.rs `draw`); while the kernel
  stepped every turn the rest never gathered, and where it skipped a turn
  the jitter's frame was drawn whole (every third frame dearer, shadows
  off). **Fixed approximately in `e6b740ab8`** by ages by square; step 3
  carries them into the tile words.
- **A shadow's end past the region** on ground falling away from a low
  light. Answered in `e6b740ab8` by the light-plane test against the
  depth's bounds; step 5 replaces that test.

The frame's caches are not each their own: every stage that stands while its
input stands keeps one `Read` (gpu/read.rs), its input byte for byte, fourteen
of them (the frame, the lists, the occlusion, the cast, the light's content
and receivers, decor grown, the air, the cases). Both failures above came
from the two places outside that model: the region, a guess at geometry, and
the resolve's history, which had no age.

## Seen wrong on main at `e6b740ab8`

- **Every frame drawn whole after any resize**, shadows on, a render scale
  other than 1 among them (`자동` changes the scale by resizing). Found by a
  probe of the frame's branch (the baseline's record): a resize while the
  depth's bounds' read-back is mapping makes the read buffer and the
  landing anew (`DepthBounds::take`) but leaves the map counted as out
  (`mapped`); the old buffer's map never lands, so nothing is copied again,
  the bounds never land, `regions` finds no bound for a moving caster, and
  every frame goes whole. Those frames wake by the timer rather than the
  kernel's next step, so the report counts few frames as moving: the "quiet
  scene" was the consequence, not the cause. The landing made anew while a
  map into the old one was out could also have been written after it was
  let go. **Fixed in `c9e822e4a`**, by the user's word once the cause was
  known: a buffer let go while its map is out is kept, with its landing and
  word, until the map lands. After two resizes and at a scale of 0.5 every
  frame is a region frame again. Step 5 still removes the path.

## What the light's cast does

The model, as the code does it (pxl-shader light.rs, pxl-render
gpu/ray/cast.rs), not as the first draft took it:

- **Whether to cast, on the CPU.** It casts in a frame where what it reads
  moved: what a tile holds (`light_content`: the light's frame and box, the
  eye's place and zooms, the world) or which tiles are read
  (`light_receivers`: the look, the actors, decor placed again). Where the
  content moved, an epoch begins and every tile is the cast's again.
- **Mark.** A receiver writes its tile's word with the cast's number: the
  world where each drawn pixel's ray met it (`light_mark_cs`), decor and the
  actors where each stands, a thread each, with a ring of tiles about it
  (`light_mark_drawn_cs`, `Light::ring`).
- **Reach.** A thread a tile reads the tiles a receiver standing there could
  read, up the way to the tallest decor (`light_reach_cs`, `Light::up`).
  Bounds that cover, not exact: a tile too many costs, one missing is a
  failure.
- **Claim by epoch.** A tile read and not yet cast in the epoch is the cast's
  (`light::reads_at`, `light::HEAD_EPOCH`).
- **Dispatch whole, pass a tile by.** Once one tile is claimed, the passes'
  args go whole (`light::ARGS_WHOLE`), and each pass returns early on a tile
  not claimed (`Map::needs`). There is no list of tiles, and no count of
  them the CPU reads.

## Proposed

The screen takes the same parts, in the renderer, and the light's map takes
its gate:

1. **A word a screen tile**, its age, the frames since it last changed.
   A tile is 32 drawn pixels a side, a multiple of the pixels' cast's
   workgroup (pxl-shader cast.rs `cast_cs`, 8 by 8).
2. **Mark.** The draws that changed mark the tiles they cover, as they were
   and as they are. In step 3, from the kernel's spheres, each box apart,
   on the CPU, which keeps the words and hands them to the GPU each frame
   (`Resolve::ages`); in step 6, where the numbers ask, from the changed
   draws drawn depth only, on the GPU. What reads the words does not change
   with the source. The casters that changed mark the light tiles they cover
   alike.
3. **Reach, the light's turned round.** A pixel whose world (its depth: the
   eye has not moved, and nothing drew there) reads through the shadow's taps
   a light tile marked this frame marks its screen tile. The light's reach
   takes the screen to the light; this takes the light to the screen. It
   reads every pixel's depth, so it runs only in a frame where a caster
   changed, a thread a square of pixels as `light_mark_cs` does.
4. **Claim by gathering, the screen's own role.** A tile changed at frame f,
   by the mark or by the reach, is claimed until it has gathered `JITTERS`
   frames since. This differs from the light's epoch by role: only the
   screen keeps a history that gathers over frames (gpu/resolve.rs), so its
   tile is done when that history has run through the jitter's sequence,
   where a light tile is done once cast. The resolve takes each tile's share
   from its own age. Every frame drawn ages the tiles, a mark or none, so
   claimed tiles retire.
5. **Whether to draw, on the CPU, as the light decides whether to cast.**
   - **Whole** where what `Read` holds moved: the frame's words, the lists
     beside what the kernel bounds, the targets, as an epoch begins for the
     light where its content moved. Every tile's age is then 0.
   - **Gated** where only what the kernel bounds moved, or tiles still
     gather: the words say which, on the GPU.
   - **Stood** where nothing moved and no tile gathers, as the light casts
     nothing where nothing it reads moved. In step 3 the words are the
     CPU's, one copy, handed to the GPU; whether a tile gathers is read off
     them. How the CPU knows it once the mark is the GPU's is step 6's to
     say.
   No other rule: no frame is drawn whole because many tiles are claimed.
   The first draft's share of the boxes the CPU marked from (`Region::of`)
   was a guess at geometry and a second copy of the words, and is gone.
6. **Draw where claimed.**
   - The compute passes (the pixels' cast) dispatched whole, each workgroup
     returning on a tile not claimed, as the light's do.
   - **The gate.** One draw instanced over every tile, an unclaimed tile
     collapsed in its vertex shader, clears the claimed tiles' color and
     depth and writes this frame's number into their stencil (State kept
     across frames, below). Every draw of the world's pass after it passes
     only where the stencil holds that number: the opaque, the blended, the
     sky. Outside the claimed tiles nothing is shaded and nothing is
     written, so the jitter, which keeps moving, moves nothing there.
   - **The passes over the whole screen are the tiles' quads.** The rays'
     screen pass, the resolve, the write-back and the finish each draw as
     the gate does, a quad a claimed tile, reading the words in their vertex
     shader. The rays' pass writes its depth and discards, so a stencil
     could not reject its fragments early; the others read the depth the
     world's pass writes, and WebGPU takes one texture as attachment and
     binding in a pass only where both its aspects are read-only; and the
     finish, at the canvas's size, takes the same tiles at its scale.
   - **The history written back** where claimed: the resolve writes the
     claimed tiles of the history its turn writes, then the write-back's
     quads copy them into the other, which stays the one read and the one
     the finish encodes.
   - **The finish into the kept picture by tile**, shown whole on the canvas.
   - **Without the stencil**, where the device has no
     `depth32float-stencil8`, the gate alone degenerates: it passes every
     pixel, so the world's pass, the rays' screen pass with it, is drawn
     whole, cleared first. The pixels' cast, their samples' raster, the
     resolve, the write-back and the finish need no stencil and stay by
     tile; an unclaimed tile is not
     resolved, written back or finished, so its history and the kept picture
     hold, and the resolve's clamp at a claimed tile's edge reads neighbours
     drawn this frame. No condition beside the gate's own.
   - **The light's map gated alike**: a word a light tile, the same
     instanced draw clearing the claimed light tiles' depth and writing its
     stencil, the casters' depth drawn through it. Until that lands, the
     map's rectangle from the casters' spheres stands.
   - **The samples' raster through a gate of its own.** The samples stand,
     as in a region frame; their boxes are drawn again where the pixels are
     cast. Its depth starts at the nearest, which no box passes, and its
     gate, one draw instanced over every tile as the world's, sets the
     claimed tiles back to the far plane, so the boxes pass the depth test
     there alone, before they are shaded (gpu/ray/raster.rs,
     raster_gate.wgsl). The raster's targets are cleared every frame, so a
     gate of depth loses nothing, where the world's would lose its unclaimed
     tiles' depth (Not taken), and it needs no stencil. The pixels' cast
     reads the raster only in the claimed tiles.

Gone with it: `Region`, the screen's scissors and rectangles, and the frame
kind `region` (step 3); the depth's bounds, their light-plane test and the
light's map's rectangle (step 5). The kernel
keeps `animated`, which wakes the turn, and its spheres only while they are
the mark's source.

## State kept across frames

Each holds only under its conditions; where one fails, the frame is drawn
whole, which renews all of it.

- **The tile words** (`changed_at`), the screen's and the map's. The CPU's
  ages in step 3; the GPU's from step 5, the number of the frame each tile
  last changed in, set by the marks' pass and the reach. Valid for one size:
  a new size makes them anew, and the targets made again move what `Read`
  holds, so that frame is whole and marks every tile. A dropped frame sets
  none; the next is whole. The numbers would come round after 2^32 frames.
- **What the reach marked, read back** (`Reach::reached`): the latest frame
  it marked a screen tile in, a word kept by the GPU, copied and mapped as
  the light's counts are; until a frame it ran in lands, the screen's tiles
  are taken to gather (`Tiles::unseen`). A dropped frame puts back the
  frame it ran in and the copy it encoded.
- **The map's stencil**, numbered as the world's (`GATE_NUMBERS`), kept
  between the frames that draw the map through its gate, cleared where it is
  drawn whole and after a dropped frame.
- **The depth's bound** (`Reach::bounds`), a square of 4 pixels a side its
  least and most depth drawn, the sky left out, for the reach. Set after the
  world's pass in the tiles it drew: the claimed in a frame through the
  gate, every one in a frame drawn whole or without the stencil, where the
  world's pass draws every tile. Valid while the eye stands, which every
  frame through the gate keeps; a new size makes it anew, and that frame is
  whole and sets every square; a dropped frame drew no depth and sets none.
- **The stencil.** In each tile, the number of the gated frame that last
  claimed it, 1 to 255 (`GATE_NUMBERS`); 0 where a frame drawn whole cleared
  it. Kept between gated frames, not cleared: the gate writes this frame's
  number into the tiles it claims and the draws test it, so a tile holding
  an older number is passed by. Cleared to 0 where the numbers come round
  (255 to 1), so no tile holds this frame's number but those it claims; by
  a frame drawn whole; after a dropped frame (the next gated frame starts
  the count again, clearing); and on a new size, whose target is new.
- **The world's color and depth.** Loaded in a gated frame, never cleared
  whole: an unclaimed tile holds what the frame that last drew it left, at
  that frame's jitter, which the resolve's clamp at a claimed tile's edge
  reads. Valid while the eye stands; any move of the view moves the frame's
  words, so the frame is whole. Without the stencil, cleared and drawn whole
  every frame, its rays at an unclaimed tile as the cast that last cast
  them left them.
- **The history.** Two, the one read and the one written; a gated frame
  writes its claimed tiles back into the one read. Valid while the view
  stands; a frame drawn whole reprojects it.
- **The kept picture** (`Surface::shown`). Finished by tile in a gated
  frame, whole in any other, and after the debug pass drew into the
  screen's depth. A new canvas size makes it anew, and that frame is whole.

Checked where each can trip, at the baseline's view: a resize (twice, in a
running page) found the rays' screen pass's group made over the rays a
resize had let go, the guest stopped; it is made where the cast prepares
now, and both resizes then drew right. The stencil's numbers come round
every 255 gated frames, many times in each 60 s run, with no tile drawn
wrong seen in the pictures taken. A dropped frame is not checked.

## Invariants

- What is drawn again holds every pixel whose light, depth or color the frame
  changes, or whose history has not gathered since; one missed is a failure.
- **Cost.** Against the baseline, below, no kind of frame slower: noon, 7:00,
  night, the clouds moving, standing still, zoomed out, and at render scales
  other than 1, `자동` among them, which changes the scale under load and so
  makes the words, the stencil and the targets anew. The gathering the
  baseline skipped where its picture was wrong is paid, bounded: a tile is
  claimed for `JITTERS` frames after its change and no longer. A walking
  actor's claim is its trail over those frames.
- **The baseline.** `c4cbb4b64` (step 2) at a fixed scale of 1.00, 3728×2098,
  Chrome on a Mac (Metal 3), [the tiles' baseline](../experiment/20261030-the-tiles-baseline.md).
  Main is no baseline at a scale other than 1: there it draws every frame
  whole (above), so "no slower" would hold of anything.
- **Devices.** The cost is held on each device; which offer
  `depth32float-stencil8` is checked on Chrome on a Mac alone, not on the
  iPhone, the Quest or Android. Nor is it checked elsewhere that the
  raster's gate of depth rejects the boxes' fragments before they are
  shaded although they may discard; on a device that tests late, the raster
  costs what it does drawn whole. Where a device lacks it, the world's pass is
  drawn whole and the rest by tile (Draw where claimed): faster than a frame
  drawn whole, but slower than the baseline's region frame wherever the
  world's pass outweighs what the region saved, so the invariant may fail
  there; to be measured once such a device is found.
- xpute unchanged: its encoder already carries stencil state, stencil
  attachments, a stencil reference and a view's aspect
  (xpute/rust/gpu/src/encode.rs).

## The stencil's cost

A pixelet change, and a wide one:

- Every pipeline drawing to the world's depth is built onto it
  (gpu/pipeline.rs `Pipelines::build_world`, `build_gate`), with its stencil
  state (gpu/api.rs `Stencil`).
- Every later read of the depth (the resolve, the finish, the depth's bounds)
  takes a depth-only view (`Surface::depth_only_view`).
- The format: `depth32float-stencil8` is an optional WebGPU feature.
  `depth24plus-stencil8` is core, but where it is 24-bit unorm (Android, the
  Quest) reversed Z loses its precision, so it is not taken.
- No fallback without a stencil is cheap: a fragment shader that may discard
  generally loses early depth and stencil for all its fragments, on tiled
  GPUs too; without the feature, the world's pass is drawn whole and the
  rest by tile.

## Order, as decided

**2026-10-30, the user's word.** The other session's line (`e6b740ab8`)
landed first, then these steps on top of it. That line holds: the shadow
map's machinery renamed sun → light, the map cast from the moon, footprint
marks as they were and as they are, the depth's bounds read back with a
light-plane test in `regions`, the finish into a kept picture, cloud
regions, ages by square in the resolve, the actor's sphere bounded by its
drawn pose, and the puck's beacon apart from its zone: step 4 with it.

That line makes main's failures right, approximately, and keeps the
rectangle. It does not make the screen and the light one shape: the screen
still decides on the CPU by a rectangle, the light on the GPU by its tile
words, and light → screen is a CPU test where screen → light is a GPU reach.
Steps 3 and 5 are what make them one. They are taken for the symmetry the
user asked, not for cost alone, and are not dropped because the picture is
right without them; only step 6 waits on the numbers. Step 5 follows step 3
directly, step 4 being done.

## Steps, each leaving the picture right

1. **Measure, on that line.** Done ([the tiles' baseline](../experiment/20261030-the-tiles-baseline.md)).
2. **The depth format and its views**: the stencil carried, no gate yet, the
   picture unchanged. Done (`c4cbb4b64`).
3. **The screen's words, claim by gathering, the CPU's part, the gate, the
   write-back**, the ages by square carried into the tile words; `Region`,
   the rectangles and the scissors removed. Done, parts 1–6. In parts, each leaving the
   picture right, the rest drawing the least rectangle about the claimed
   tiles until its part lands:
   1. the words marked tile by tile, each sphere's box and each shadow's
      square apart, not their one rectangle;
   2. the resolve and the write-back as the tiles' quads;
   3. the finish into the kept picture as the tiles' quads;
   4. the world's pass through the stencil the gate writes, the rays'
      screen pass as the tiles' quads;
   5. the pixels' cast returning on an unclaimed tile, and the samples'
      raster through a gate of its own (done, the raster's a gate of
      depth);
   6. `Region`, the scissors, the rectangles and the CPU's whole-by-share
      rule removed (done): what moved marks the tiles, a box each
      (`resolve::Mark`), and the frame is drawn whole, gated or stood, the
      frame kind `region` now `gated`.
   The depth's bounds are not reworked beyond the tiles: their squares are
   the screen's tiles, so with the rectangle gone they take the claimed ones,
   dispatched whole, the rest passed by; step 5 removes them.
4. **The spheres tightened.** Done in `e6b740ab8`.
5. **The reach from the light's tiles to the screen, and the light's map
   gated**; the light-plane test, the depth's bounds and their read-back,
   and the map's rectangle removed. In parts, each leaving the picture
   right:
   1. **The screen's words the GPU's** (the user's word): a word a tile,
      the number of the frame it last changed in, as a light tile's word
      names a cast; the CPU hands its marks, a box each, and whether the
      frame is whole, and a pass stamps the tiles they cover (every one
      where whole); claimed while the frames since are fewer than
      `JITTERS`. The CPU knows whether any tile gathers as the light knows
      whether to cast, from what it handed: fewer than `JITTERS` frames
      since a frame marked anything. The picture as in step 3. Done.
   2. **The light's map gated**: the screen's words, uniform and marks
      made one type (`Tiles`), the map an instance of it, claimed the frame
      it changed alone, as it gathers nothing; its depth carries the gate's
      stencil; a word a tile of the map, set by the casters that changed, as
      they were and as they are, handed as the actors are; the gate clears the
      claimed tiles' depth and raises their stencil, the world's depth laid
      in as the tiles' quads (it writes its depth, as the rays' screen pass
      does), the casters drawn through the stencil. The map's rectangle and
      its scissor removed; without the stencil the map is drawn whole. Done.
   3. **The reach**: in a frame where a tile of the map was stamped, a
      thread a square of the screen reads each pixel's depth, as the world's
      pass left it, the eye standing, takes its texel in the map and the
      taps about it, and stamps its screen tile where any tap lies in a tile
      stamped this frame. The light-plane test, `stood`, the depth's bounds
      and their read-back removed, and the shadow's squares with them.
6. **The mark from the changed draws drawn depth only**, if the numbers ask;
   the kernel's spheres removed with it, and how the CPU knows whether any
   tile gathers said.
   Taken by the user's word (2026-10-30), in parts, each leaving the
   picture right, once the claimed area was measured to be the spheres'
   size (some 520 tiles a frame, the baseline's record):
   1. **What each bounded draw was drawn with, kept**: the renderer keeps
      the last frame's record and uniform of every draw outside `BESIDE`,
      and a skinned draw's joint palette, copied before the frame's upload
      writes it again.
   2. **The kernel hands the draws that changed**, not their spheres
      (pxl-kernel region.rs); the spheres, `unbounded` and the host's marks
      from them removed. Taken after the screen's side has its numbers.
      Done (2026-10-31): every bounded draw, decor, an impostor and the
      puck's zone and beacon beside the actors, marks by its drawn self;
      the kernel hands the keys of the draws it bounds and the renderer
      tells which changed against what it kept, as the actors' did.
   3. **The changed draws drawn as they were and as they are**, their
      vertices as their depth pipelines lay them, through a fragment that
      sets the screen tile's word to the frame's number and keeps the
      frame's number for the read-back, the world's depth attached read
      only; draw by draw, the skinned first, its sphere's mark kept for
      the rest until each has its own. Done for the actors, part 1 with
      it: alone it claims no less (the claim was the reach's).
   4. **The same from the light** for the casters, setting the map's
      tiles' words. Done for the actors: their drawn selves set the words
      the reach reads, about 2 ms under the commit before with the
      companion walking (the baseline's record); the map's gate and lay
      draw a tile of their spheres' rectangles each, the claimed alone
      (`Tiles::beside`), so the map draws its claimed tiles alone, as the
      screen does.
   5. **The host knows whether any tile gathers** from the read-back word,
      the reach's widened to the marks.

## Not taken

- **A list of the claimed tiles, drawn a quad a tile, instanced by indirect
  draw** (the first draft). The light's cast keeps no list; whole args and a
  tile passed by are what it does and what holds.
- **The opaque draws left to fail the depth test outside the tiles.** The
  jitter moves in a partial frame, and the opaque pipelines test greater, so
  at edges and on slopes they pass and shade outside. (The light's map has
  no jitter, so its casters drawn again would fail there exactly; but they
  would rasterize all of decor over the whole map, so it takes the gate.)
- **The finish whole, no kept picture.** Weighed first; the other
  session's line then kept the picture at the canvas's size, which the
  tiles finish as they finish everything else.
- **The passes over the whole screen through the stencil.** They would
  attach the depth they read, read-only in both aspects; the finish, at the
  canvas's size, could not take the world's stencil; and the rays' pass,
  which writes its depth and discards, would be shaded whole under it
  (14.9 ms against 5.6 as quads, the baseline's record).
- **A frame drawn whole where nearly every tile is claimed.** A CPU guess at
  geometry from the boxes it marks from, and a second copy of the words; the
  light has no such rule.
- **The region kept, tightened alone.** Main already ages its squares, so
  the history's want is met either way; what it would not meet is the
  symmetry: two shapes, the rectangle and the light's tiles, and a CPU test
  for light → screen where screen → light is a GPU reach.
- **Every frame drawn whole where the device has no stencil.** Slower than
  main's region frames on those devices, which may be the weakest; only the
  gate needs the stencil, so only the world's pass need go whole.
- **A gate of depth where there is no stencil**: the unclaimed tiles' depth
  set nearest, so every draw fails there. It loses those tiles' depth, which
  the resolve's nearest depth about a claimed tile's edge pixel reads, so
  that pixel's history would be found at the wrong place.
- **Reworking the depth's bounds beyond the tiles.** They serve only the
  light-plane test, which step 5 removes; their squares, being the screen's
  tiles, take the claimed ones once the rectangle is gone, and no more.
- **The samples' raster through the stencil, as the world's pass.** The
  boxes' fragments discard where a pixel's ray does not enter across the
  face, and write their depth; the stencil was tested after they were
  shaded, and the gate's writes added to it: 4.3 ms at the p95, against 2.8
  drawn whole and 0.5 within the rectangle (the baseline's record).
- **Each of the raster's fragments returning on an unclaimed tile**, as the
  pixels' cast's workgroups do. The boxes overlap many times over, and each
  fragment reading its tile's age cost nearly what shading it did: 2.4 ms at
  the p50, the claimed tiles about a tenth of the screen.

## Open

- The stencil's format on the iPhone, the Quest and Android, and whether
  the raster's gate of depth rejects early there.
- Part 4 of step 3 costs about 1.2 ms over parts 1–3 at the baseline's
  quiet view, not yet explained against what the structure predicts. At the
  afternoon's real hour, a busier window, step 3 whole is 4.6 ms under step
  2 at the p50 (the baseline's record); the quiet view, 7:00, night, the clouds, zoomed out
  and other scales are still to be taken against the baseline.
- Whether the mark from the kernel's spheres already meets the invariants,
  leaving step 6 unneeded.

## Proposed after the steps (2026-10-31)

Where the steps leave a frame, measured against the plan's start
(`5553036df`) in one window (the baseline's record): a still frame's GPU
some 13.6 ms then, 5.5 now; a frame drawn whole while the camera orbits
some 25 ms then and now, the 4 ms the light's footprints put on it
(`e6b740ab8`) taken back by the light held while only the eye moves
(`light_detail_cs`), its marks reading each pixel once, and the footprints
taken where the eye stands (`light::HEAD_FOOTPRINT`). The screen and the
light are one shape; what is left of a moving frame is not the symmetry's.
Proposals only, none taken; each a decision of its own:

- **The eye's samples and what they miss.** Orbiting, 1.6 % of the pixels
  find no sample's cell and walk alone, some 16 nodes each, some 5 ms
  (`miss`, its list well within `observed::MARKS`); the eye's cast in all
  some 13 ms of a moving frame's 24. Fewer walked alone means samples
  closer where they miss (edges, thin things), which the samples' own cost
  pays for. Both observers share the passes (pxl-shader observer.rs), so a
  change holds for the light alike.
- **The screen reprojected while the eye moves.** The light now holds its
  tiles across the eye's motion, being fixed in the world; the screen,
  fixed to the view, draws whole each frame. Last frame's rays moved to
  the new view, with what they uncover cast anew, would give the screen the
  same holding, by an approximation the picture must be held to.
- **The light's marks only where the eye cast anew.** Orbiting, the marks
  are some 4 ms (`light_beam` less the beam and the cells), every drawn
  pixel's each moving frame. Marked only where the screen's tiles changed,
  the rest standing as marked, they would follow the eye's own change.

To measure any of them: `pxl.profiler.drive` and the busy time by frame
kind (`gpu_busy_kinds`), A and B by turns in one sitting; a run alone has
read the same commit 4 ms apart, and some runs the GPU at half its speed
across every span, with no thermal or power record of it.

## Taken after the steps (2026-10-31)

The user's word, each in its scope; measured in the tiles' baseline's
record (the round after the steps):

- **A held light tile is cast again exactly where what its walks asked
  moved**: each pass tells the least margin the eye may move before a node
  it asked changes its zooms, a tile keeps the least and the eye it was
  claimed at, and `light_detail_cs` takes it back where the eye moved past
  it (`cast::Margin`, `light::holds`), in place of a slot's stop's zoom,
  which let a coarse cell's changed look stand as a shadow with no caster.
- **The occlusion takes the shadows' shape**: found from the world's own
  depth between the world pass's two halves, held where what it reads
  stands, laid over the claimed tiles; its prepass and depth gone, a change
  in how it looks accepted.
- **The air's volume and exposure each found again only where what it reads
  moved.**

