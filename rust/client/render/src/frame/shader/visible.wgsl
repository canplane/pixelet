// pxl-render/visible.wgsl

// A ray off the visible voxel the raster kept where it lies, the eye's pixels' (cast.wgsl), a
// voxel of its record, and the light's slots' (light.wgsl), a voxel of the light's: the cell it
// takes is the voxel's, where the ray enters the voxel's box through a side its box draws, one
// some ray entered it through (`raster::accepts`), tested again exactly where a GPU's f32 may
// part the two (pxl-engine `cast::entry`, `fill_bounds`); the sky where the samples about it
// each went on to their reach; and the counts of what each took.

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

/// One more of the counts' lane `lane` where `on` (`count::REUSED` on; list.wgsl `MissArgs`).
fn add_cnt(on: bool, lane: u32) {
  if (on) {
    atomicAdd(&args.counts[lane - COUNT_REUSED], 1u);
  }
}
