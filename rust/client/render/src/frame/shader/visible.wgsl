// pxl-render/visible.wgsl

// What a ray takes from the visible voxel the raster kept at its place, for the eye's pixels
// (cast.wgsl) and the light's slots (light.wgsl), each from its own observer's record. The ray
// takes the voxel's cell where it enters the voxel's box through a side the box draws, one some
// ray entered it through (pxl-engine `raster::accepts`). That test is made again here, exactly,
// because a GPU's f32 may part the raster from pxl-engine's test (`cast::entry`,
// `fill_bound`). The ray takes the sky where the samples around it each went on to their
// reach. Any other ray is a hole, listed for the holes' queue (queue.wgsl). Each observer
// counts what its rays took.

/// The holes' count and the count lanes, laid out as pxl-engine `ray::MISS_ARGS_WORDS` lays
/// them: how many holes are listed (`MISS_ARGS_CNT`), then what the rays took
/// (`MISS_ARGS_CNTS`). build.rs checks this struct's size against pxl-engine's.
struct MissCount {
  cnt: atomic<u32>,
  cnts: array<atomic<u32>, CNT_RAY_CNT>,
}

/// Where a ray enters its visible voxel's cell, the axis it enters across, and whether it
/// enters within its reach across a face the box draws.
struct Entry {
  t: f32,
  axis: u32,
  enters: bool,
}

/// Where the ray `aim` enters the cell of the node at `zoom` and `corner`, in the root `roots`
/// counted from the origin's. The cell is inset to the share its `fill` keeps (pxl-engine
/// `cast::asset_box`, `fill_bound`). The ray must enter through one of the sides in `faces`,
/// the sides the box draws (pxl-engine `raster::faces`, `ray::face`).
fn visible_cell(aim: Aim, zoom: u32, corner: vec3<u32>, roots: vec3<i32>, fill: u32, faces: u32) -> Entry {
  let dir = ray_dir(aim.dir);
  let og = vec3<u32>(aim.origin) + vec3<u32>(0u, Y_START, Z_START);
  let lo = units(corner, og, roots);
  let hi = far_of(corner, under(zoom), og, roots);
  let inset = (hi - lo) * (0.5 * (1.0 - fill_side(fill)));
  let b = span_of(dir, aim.inv, lo + inset, hi - inset, 0.0, aim.t_max);
  let axis = crossing(dir, b.near, true);
  // The side the ray enters through, numbered as the bits of `faces` are (pxl-engine
  // `ray::face`).
  let face = 2u * axis + select(0u, 1u, dir[axis] > 0.0);
  let enters = min(aim.t_max, smallest(b.far)) > max(0.0, largest(b.near)) && ((faces >> face) & 1u) != 0u;
  return Entry(past_origin(b.near[axis]), axis, enters);
}

/// Whether the sample at word `k` went on to its reach, meeting nothing (pxl-engine
/// `beam::SAMPLE_NONE`).
fn went_on(k: u32) -> bool {
  return beams[k + SAMPLE_NODE + NODE_ZOOM] == SAMPLE_NONE;
}

/// Adds one to count lane `lane` where `on`. The lanes in `args` begin at pxl-engine
/// `count::VOXEL`.
fn add_cnt(on: bool, lane: u32) {
  if (on) {
    atomicAdd(&args.cnts[lane - CNT_VOXEL], 1u);
  }
}

/// Lists pixel or slot `p` as a hole, its column in the low 16 bits and its row in the high.
/// The holes' list lies at word `at` of `beams`, its count first and its entries after it
/// (pxl-engine `ray::holes_at`, `light::holes_at`), with room for `cap`. The entry's place is
/// taken from the count in `args`, and a place past the room is not written.
fn list_hole(at: u32, cap: u32, p: vec2<u32>) {
  let k = atomicAdd(&args.cnt, 1u);
  if (k < cap) {
    beams[at + 1u + k] = p.x | (p.y << 16u);
  }
}
