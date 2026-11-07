// pxl-render/visible.wgsl

// A ray off the visible voxel the raster kept where it lies, the eye's pixels' (cast.wgsl), a
// voxel of its record, and the light's slots' (light.wgsl), a voxel of the light's: the cell it
// takes is the voxel's, where the ray enters the voxel's box through a side its box draws, one
// some ray entered it through (`raster::accepts`), tested again exactly where a GPU's f32 may
// part the two (pxl-engine `cast::entry`, `fill_bounds`); the sky where the samples about it
// each went on to their reach; any other a hole, listed for the holes' queue (queue.wgsl); and
// the counts of what each took.

/// The holes' count and the counts' lanes (pxl-engine `ray::MISS_ARGS_WORDS`): how many are
/// listed (`MISS_ARGS_CNT`), and what was counted (`MISS_ARGS_COUNT`). build.rs holds its words
/// to pxl-engine's.
struct MissArgs {
  cnt: atomic<u32>,
  counts: array<atomic<u32>, COUNT_RAYS>,
}

/// Where a ray enters its visible voxel's cell, across which axis, and whether it enters it
/// across a face its box draws.
struct Entry {
  t: f32,
  axis: u32,
  enters: bool,
}

/// Where the ray `aim` enters the cell of the node of `zoom` at `corner` in the root `roots`
/// from the origin's, inset by the share its `fill` keeps (`asset_box`, `fill_bounds`), through
/// one of the sides `faces`, its box's (`raster::BOX_FACES`, `ray::face`).
fn visible_cell(aim: Aim, zoom: u32, corner: vec3<u32>, roots: vec3<i32>, fill: u32, faces: u32) -> Entry {
  let dir = ray_dir(aim.dir);
  let og = vec3<u32>(aim.origin) + vec3<u32>(0u, Y_START, Z_START);
  let lo = units(corner, og, roots);
  let hi = far_of(corner, under(zoom), og, roots);
  let inset = (hi - lo) * (0.5 * (1.0 - side(fill)));
  let b = span_of(dir, aim.inv, lo + inset, hi - inset, 0.0, aim.t_max);
  let axis = crossing(dir, b.near, true);
  // The side it enters through, a bit of `faces` (pxl-engine `ray::face`).
  let face = 2u * axis + select(0u, 1u, dir[axis] > 0.0);
  let enters = min(aim.t_max, smallest(b.far)) > max(0.0, largest(b.near)) && ((faces >> face) & 1u) != 0u;
  return Entry(past_origin(b.near[axis]), axis, enters);
}

/// Whether the sample at word `k` went on to its reach, meeting nothing (`beam::SAMPLE_NONE`).
fn went_on(k: u32) -> bool {
  return beams[k + SAMPLE_NODE + NODE_ZOOM] == SAMPLE_NONE;
}

/// One more of the counts' lane `lane` where `on` (`count::REUSED` on).
fn add_cnt(on: bool, lane: u32) {
  if (on) {
    atomicAdd(&args.counts[lane - COUNT_REUSED], 1u);
  }
}

/// Pixel or slot `p` listed a hole, its column under its row, among the holes whose count lies
/// at word `at` of `beams`, their entries after it (pxl-engine `ray::holes_at`,
/// `light::holes_at`), its place counted in `args`.
fn list_hole(at: u32, p: vec2<u32>) {
  beams[at + 1u + atomicAdd(&args.cnt, 1u)] = p.x | (p.y << 16u);
}
