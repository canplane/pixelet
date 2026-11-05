// pxl-render/visible.wgsl

// A ray off the visible sample the raster kept where it lies, the eye's pixels' (cast.wgsl)
// and the light's slots' (light.wgsl): the cell it takes is that sample's, where the ray enters
// that sample's box across the axis the sample entered by, tested again exactly where a GPU's
// f32 may part the two (pxl-engine `cast::entry`, `fill_bounds`); the sky where the samples
// about it each went on to their reach; and the counts of what each took.

/// Where a ray enters its visible sample's cell, across which axis, and whether it enters it
/// as the sample did.
struct Entry {
  t: f32,
  axis: u32,
  enters: bool,
}

/// Where the ray `aim` enters the box of the sample at word `k` (`beam::SAMPLE_WORDS`), its
/// cell inset by the share it keeps (`asset_box`, `fill_bounds`).
fn visible_entry(aim: Aim, k: u32) -> Entry {
  let node = k + SAMPLE_NODE;
  let zoom = beams[node + NODE_ZOOM];
  let corner = vec3<u32>(beams[node + NODE_CORNER], beams[node + NODE_CORNER + 1u], beams[node + NODE_CORNER + 2u]);
  let roots = vec3<i32>(i32(beams[node + NODE_ROOTS]), i32(beams[node + NODE_ROOTS + 1u]), i32(beams[node + NODE_ROOTS + 2u]));
  let fill = (beams[k + SAMPLE_ASSET] >> FILL_SHIFT) & (EIGHTHS - 1u);
  let dir = ray_dir(aim.dir);
  let og = vec3<u32>(aim.origin) + vec3<u32>(0u, Y_START, Z_START);
  let lo = units(corner, og, roots);
  let hi = far_of(corner, under(zoom), og, roots);
  let inset = (hi - lo) * (0.5 * (1.0 - side(fill)));
  let b = span_of(dir, aim.inv, lo + inset, hi - inset, 0.0, aim.t_max);
  let axis = crossing(dir, b.near, true);
  let enters = min(aim.t_max, smallest(b.far)) > max(0.0, largest(b.near)) && axis == beams[k + SAMPLE_AXIS];
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
