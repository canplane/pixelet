# The drawn resolution's `auto`, over the pixels and the camera grid's lattice

**2026-11-02** — recorded with the user. The span split is `8727d4498f`, on
`07a06b1604`, on `641e855010`; the second knob is built on it. The commits are
named as `original` keeps them.

## Against the ray cast

`auto` came in at `4bcb287d0` (2026-09-25), when the renderer was a raster:
it lowered the world's resolution "so the fill, the GPU's largest cost, gives
way before the frame rate does". The user: `auto` is adaptive budget control,
congestion-control style, and its controller stands (the budget the display's
interval, no tighter than 1/60 s; a step down while over; a step up a probe
whose failure doubles the wait before the next); its premise, that the fill is
the GPU's largest cost, predates the ray cast. The heavy passes now run on the
camera grid's angular cells and the light's map, which no drawn resolution
shrinks, so the controller reads only what the resolution buys back.

- Each GPU span says whether every pass it times runs a drawn pixel at a time
  (span.json `per_px`, `GpuSpanName::PER_PX`, `07a06b1604`), and a span with
  passes of both kinds is split: `sraster` into the boxes written over the
  samples (`sample_box`) and drawn into the visibility buffer (`sraster`);
  `mark` into the screen's (`mark`), the light's (`light_mark`) and the
  propagation to the screen's tiles, a thread a square of the drawn pixels
  (`propagate`); `light_beam` gives up the light's tiles marked where the
  eye's rays meet the world, a thread a drawn pixel (`light_receivers`). A
  span times from its first pass to its last, and the renderer encodes each
  span's passes together.
- The markers and the finish draw at the display's pixels at any scale, their
  text sharp, and are no part of the signal (`marker`, `finish`).

## The second knob: the camera grid's lattice

The user approved a second knob for `auto`: the lattice the camera grid's beam
rays and cells are laid on, `beam::Grid::s` a power of two of cells
(`grid::CELL`) a side, sharing the one budget with the scale. The cell keeps
the angle every ray asks detail at (`cell_zoom_at`, `CELLS_A_RAD`), so every
screen observes the same world; until now it spaced the beam rays as well.

- Tried first, in the design: the lattice from 1 to 2 cells in √2 steps,
  `SAMPLE` held a lattice cell a side. Read against the samples' math, it
  loses nodes: at twice the cell the samples are every other ray of the
  cell's own, and a node seen only as a strip narrower than half a cell
  (behind an occluder's edge) can go unhit, its pixels taking the box behind
  it (raster.rs `visible_cell`) where the cell's lattice drew it. A node seen
  whole is hit at either lattice, its inscribed ball wider than the coarser
  samples' diagonal.
- The user chose to hold the samples per observed node: `SAMPLE` a cell a
  side at every lattice, so the samples are the very rays of the cell's
  lattice, bit for bit, and as many (`beam::Grid::sample`, `sample_cell`,
  the test `samples_same_every_lattice`). That needs the lattice a power of
  two of cells, so the √2 step is dropped: the lattice is 1 or 2
  (`grid::LATTICE_MAX`). The knob buys back the beam rays and the cells'
  starts, a quarter of them at 2; a sample walks from its coarser cell's start
  the farther. The beams' `narrow` reads the lattice (pxl-shader eye.rs
  `eye_origin`), so a beam ray hands on no farther than the pixels about it
  enter.
- The signal: span.json `per_cell` on the spans that run over the lattice's
  corners or cells, `beam` and `cell`; `sample`, `sample_box` and `face` are
  as many at every lattice and stay in the rest (pxl-render gpu/renderer.rs
  `cell_ms` beside `px_ms`).
- One judge for both (resolution.rs): the load is F + P + G, P the per-pixel
  spans', G the lattice's, F the rest of the GPU's busy time; the step above
  is F + P·(its share over this one)² + G·(this lattice over its)² against
  the budget. Before, a timed step up checked P alone. A knob a step a
  settling window; down, the scale to `SCALE_MIN` first, then the lattice;
  up, the lattice first, then the scale. Without timestamps only the scale
  moves; `fixed` and `low` lay the cell's own lattice. A third knob, the rate
  turns go in at, was added on 2026-11-07, first down and last up
  ([a ray ends in its frame](./20261107-a-ray-ends-in-its-frame.md)).
- A lattice changed casts in full: the eye's grid words change, as a moved
  eye's do (pxl-render gpu/ray/cast.rs `eye_moved`). A read timed at another
  lattice is no sample (`FrameStat::gpu_lattice`); the perf frame's
  `lattice` column records the lattice each frame laid.
- The judge's constants the knobs share are `RES_` (the windows, the band,
  the budget's floor, the settling); those of the scale alone keep `SCALE_`
  (its least and its steps, the misses and probes judged untimed).

## Against the light's band

`9cd5f6ea83` landed under the stack: the light runs the eye's `beam_cs` and
`sample_cs`, and a light tile asks the world at its band
([a light tile asks at its band](./20261102-a-light-tile-asks-at-its-band.md)). Read
again against it, the knob's premises hold:

- The light's passes take no word of the camera grid: every lattice method
  of the observer the light's head names is the light map's (pxl-shader
  pass_observer.rs `PassObserver`), its beam rays `light::BEAM` texels apart
  (`Light::spacing`) and its samples `param::light::SAMPLE`; its recast reads
  the eye's range, place and look, never `eye::GRID` (pxl-render
  gpu/ray/cast.rs `light_content`, `light_receivers`). A lattice changed
  leaves the light's cast as it was, its rays the same.
- The band is the cell's zoom at the tile column's nearest point to the eye,
  held to the eye's zooms (`Light::band`, `Detail::at`, `cell_zoom_at`); the
  lattice moves neither the cell nor `eye::RANGE`.
- The eye's walks ask one `Detail` each frame, its spacing the lattice's side
  (eye.rs `eye_origin`), so `Detail::alike` holds between the eye's beam
  rays, cells, samples and pixels at either lattice, and every cell resumes.
- `beam` and `cell` time the eye's passes alone; the light's run in
  `light_beam` and `light_sample`, on its map's lattice, and stay in F.

## Open

- Measured on no GPU: how much of the frame `beam` and `cell` are where the
  budget runs over, and how much farther the samples walk at twice the cell.
  The comparison against a base (`script/compare-perf.py`) was
  not run on the second knob.
- A sample's hit the same at either lattice is argued, not tested: the test
  holds its ray and its cell; its walk from the coarser cell's start is not
  run against the cell's own on a world.
- The steps view (the nodes a pixel's walk visited, `SAMPLE_VISITS`) counts
  the beam ray's visits, which the lattice changes. The view was removed on
  2026-11-06 ([the frame is what it must draw](./20261106-the-frame-is-what-it-must-draw.md)).
