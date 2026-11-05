// pxl-render/cast.wgsl

// Each drawn pixel's ray off the visible sample the raster kept at it (pxl-engine `raster`;
// gpu/ray/raster.rs), or the sky, a thread a pixel of the tiles claimed (cast_tiles.wgsl), as
// light.wgsl's `visible_entry` and `sky` take a slot's. The cell
// it takes is the visible sample's, where its ray enters that sample's cell across the axis the
// sample entered by, tested again exactly where a GPU's f32 may part the two (`cast::entry`),
// with what lies about its face, the sample's; the sky where no sample about it met anything.
// Any other pixel takes no walk here: its first word is the mark the walk again goes by
// (`ray::MISS`; miss_hit.wgsl), and it is listed for that walk (`node_cache::MARKS_AT`) by a plain
// write, its place taken off the count in its dispatch's words (`ray::MISS_ARGS_CNT`); one past
// the list's room has the walk over every pixel dispatched instead.

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
@group(0) @binding(8) var<storage, read> beams: array<u32>;
@group(0) @binding(9) var<storage, read_write> node_cache: array<u32>;
/// Each drawn pixel's visible sample (gpu/ray/raster.rs): its number (`raster::id_of`), 0
/// where it kept none, read as an array of one layer.
@group(0) @binding(13) var visibility: texture_2d_array<u32>;
/// The walk again's words: its dispatches, the count it lists by and the counts' lanes
/// (`ray::MISS_ARGS_WORDS`).
@group(0) @binding(14) var<storage, read_write> marks: array<atomic<u32>>;
/// The tiles claimed, this pass's dispatch ahead of them (cast_tiles.wgsl).
struct CastTiles {
  args: array<u32, 3>,
  at: array<u32>,
}
@group(0) @binding(30) var<storage, read> claimed: CastTiles;

/// `v` added to the counts' lane `lane` (pxl-engine `count::REUSED` on), where it counts
/// (`ray::MISS_ARGS_COUNT`).
fn add_cnt(on: bool, lane: u32) {
  if (on) {
    atomicAdd(&marks[MISS_ARGS_COUNT + lane - COUNT_REUSED], 1u);
  }
}

/// One more missed pixel listed for its walk again: its place off the count, the rows the list
/// fills raised to hold it by the place that opens each; the first past the list's room has
/// the walk over `whole` workgroups dispatched instead (pxl-engine `ray::MISS_ARGS_WORDS`).
fn enlist(whole: vec2<u32>) -> u32 {
  let at = atomicAdd(&marks[MISS_ARGS_CNT], 1u);
  let per = MISS_LANES * MISS_ACROSS;
  if (at < MARKS) {
    if (at % per == 0u) {
      atomicMax(&marks[MISS_ARGS_LIST + 1u], at / per + 1u);
    }
  } else if (at == MARKS) {
    atomicMax(&marks[MISS_ARGS_WHOLE], whole.x);
    atomicMax(&marks[MISS_ARGS_WHOLE + 1u], whole.y);
  }
  return at;
}

/// The first word of sample `s`'s hit (`Grid::sample_at`).
fn sample_word(s: vec2<u32>) -> u32 {
  return eye_word(GRID + GRID_SAMPLES_AT) + (s.y * eye_word(GRID + GRID_SAMPLES) + s.x) * SAMPLE_WORDS;
}

/// Whether sample `s` went on to its reach, meeting nothing (`went_on`).
fn went_on(s: vec2<u32>) -> bool {
  return beams[sample_word(s) + SAMPLE_NODE + NODE_ZOOM] == SAMPLE_NONE;
}

/// The first of the four samples about the screen's point at the tangents `at`
/// (`Grid::sample_before`).
fn sample_before(at: vec2<f32>) -> vec2<u32> {
  let v = vec2<f32>(at.x + eye_f32(TAN), eye_f32(TAN + 1u) - at.y);
  return vec2<u32>(max(floor(v * SAMPLE_SPACING_INVERSE), vec2<f32>(0.0))) + vec2<u32>(NEAR_SAMPLES);
}

/// The ray's words at pixel `p`.
fn write_ray(p: vec2<u32>, w: vec4<u32>) {
  for (var i = 0u; i < 4u; i += 1u) {
    textureStore(rays, p, i, vec4<u32>(w[i], 0u, 0u, 0u));
  }
}

@compute @workgroup_size(CAST_SIDE, CAST_SIDE, 1)
fn cast_cs(@builtin(workgroup_id) w: vec3<u32>, @builtin(local_invocation_id) l: vec3<u32>) {
  let tile = claimed.at[w.x];
  let p = vec2<u32>(tile & 0xffffu, tile >> 16u) * SCREEN_TILE + w.yz * CAST_SIDE + l.xy;
  let size = textureDimensions(rays);
  if (p.x >= size.x || p.y >= size.y) {
    return;
  }
  let px = px_tan(p, size.xy);
  let aim = eye_aim(px);
  let dir = ray_dir(aim.dir);
  let visible = textureLoad(visibility, vec2<i32>(p), 0i, 0i).x;
  let steps = eye_word(ORIGIN + 3u) != 0u;
  let on = node_cache[COUNT_ON] != 0u;
  if (visible != 0u && (visible & VISIBLE_LACK) == 0u) {
    // The cell off its visible sample, where its ray enters that sample's box across the axis
    // the sample entered by (`visible_entry`, `visible_cell`, `cast::entry`).
    let n = eye_word(GRID + GRID_SAMPLES);
    let s = (visible & ~VISIBLE_LACK) - 1u;
    let k = sample_word(vec2<u32>(s % n, s / n));
    let node = k + SAMPLE_NODE;
    let zoom = beams[node + NODE_ZOOM];
    let corner = vec3<u32>(beams[node + NODE_CORNER], beams[node + NODE_CORNER + 1u], beams[node + NODE_CORNER + 2u]);
    let roots = vec3<i32>(i32(beams[node + NODE_ROOTS]), i32(beams[node + NODE_ROOTS + 1u]), i32(beams[node + NODE_ROOTS + 2u]));
    let asset = vec2<u32>(beams[k + SAMPLE_ASSET], beams[k + SAMPLE_ASSET + 1u]);
    // Its box as drawn from the eye (`asset_box`, `fill_bounds`).
    let og = vec3<u32>(aim.origin) + vec3<u32>(0u, Y_START, Z_START);
    let lo = units(corner, og, roots);
    let hi = far_of(corner, under(zoom), og, roots);
    let inset = (hi - lo) * (0.5 * (1.0 - side((asset.x >> FILL_SHIFT) & (EIGHTHS - 1u))));
    let b = span_of(dir, aim.inv, lo + inset, hi - inset, 0.0, aim.t_max);
    let axis = crossing(dir, b.near, true);
    let enters = min(aim.t_max, smallest(b.far)) > max(0.0, largest(b.near));
    if (enters && axis == beams[k + SAMPLE_AXIS]) {
      // Its words, with what lies about its face and the nodes its walk visited, the sample's
      // (`word`, `ray::first_word`).
      add_cnt(on, COUNT_REUSED);
      let face = 2u * axis + select(0u, 1u, dir[axis] > 0.0);
      let fill = (asset.x >> FILL_SHIFT) & (EIGHTHS - 1u);
      write_ray(p, vec4<u32>(
        (asset.x & VAL_MASK) | (face << RAY_FACE_SHIFT) | (zoom << RAY_ZOOM_SHIFT) | (fill << RAY_FILL_SHIFT),
        bitcast<u32>(past_origin(b.near[axis])),
        select(asset.y, beams[k + SAMPLE_VISITS], steps),
        beams[k + SAMPLE_FACES],
      ));
      return;
    }
  }
  if (visible == 0u) {
    // The sky where none of the four samples about it met anything (`sky`), taken without a
    // walk of its own, as a thing thinner than their spacing goes unseen.
    let s = sample_before(px);
    if (went_on(s) && went_on(s + vec2<u32>(1u, 0u)) && went_on(s + vec2<u32>(0u, 1u)) && went_on(s + vec2<u32>(1u))) {
      add_cnt(on, COUNT_SKY);
      write_ray(p, vec4<u32>(0u, 0u, select(0u, beams[sample_word(s) + SAMPLE_VISITS], steps), 0u));
      return;
    }
  }
  // Neither: marked and listed for its walk (miss_hit.wgsl), which reads and writes the
  // node cache.
  add_cnt(on, COUNT_WALKED);
  textureStore(rays, p, 0u, vec4<u32>(RAY_MISS, 0u, 0u, 0u));
  let at = enlist((size + vec2<u32>(CAST_SIDE - 1u)) / CAST_SIDE);
  if (at < MARKS) {
    node_cache[MARKS_AT + at] = (p.y << 16u) | p.x;
  }
}
