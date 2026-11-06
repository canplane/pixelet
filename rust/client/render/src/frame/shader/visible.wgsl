// pxl-render/visible.wgsl

// A ray off the visible voxel the raster kept where it lies, the eye's pixels' (cast.wgsl), a
// voxel of its record, and the light's slots' (light.wgsl), a voxel of its cast: the cell it
// takes is the voxel's, where the ray enters the voxel's box across a face its box draws, an
// axis some ray entered it by (`raster::accepts`), tested again exactly where a GPU's f32 may
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
/// from the origin's, inset by the share its `fill` keeps (`asset_box`, `fill_bounds`), across
/// one of the axes `faces`, its box's (`raster::BOX_FACES`).
fn visible_cell(aim: Aim, zoom: u32, corner: vec3<u32>, roots: vec3<i32>, fill: u32, faces: u32) -> Entry {
  let dir = ray_dir(aim.dir);
  let og = vec3<u32>(aim.origin) + vec3<u32>(0u, Y_START, Z_START);
  let lo = units(corner, og, roots);
  let hi = far_of(corner, under(zoom), og, roots);
  let inset = (hi - lo) * (0.5 * (1.0 - side(fill)));
  let b = span_of(dir, aim.inv, lo + inset, hi - inset, 0.0, aim.t_max);
  let axis = crossing(dir, b.near, true);
  let enters = min(aim.t_max, smallest(b.far)) > max(0.0, largest(b.near)) && ((faces >> axis) & 1u) != 0u;
  return Entry(past_origin(b.near[axis]), axis, enters);
}

/// `visible_cell` of the light's voxel at word `k` (`beam::VOXEL_WORDS`).
fn visible_entry(aim: Aim, k: u32, faces: u32) -> Entry {
  let node = k + VOXEL_NODE;
  let corner = vec3<u32>(beams[node + NODE_CORNER], beams[node + NODE_CORNER + 1u], beams[node + NODE_CORNER + 2u]);
  let roots = vec3<i32>(i32(beams[node + NODE_ROOTS]), i32(beams[node + NODE_ROOTS + 1u]), i32(beams[node + NODE_ROOTS + 2u]));
  let fill = (beams[k + VOXEL_ASSET] >> FILL_SHIFT) & (EIGHTHS - 1u);
  return visible_cell(aim, beams[node + NODE_ZOOM], corner, roots, fill, faces);
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
