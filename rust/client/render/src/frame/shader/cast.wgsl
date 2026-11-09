// pxl-render/cast.wgsl

// Finds each drawn pixel's ray without a walk, one thread a pixel of the tiles claimed
// (cast_tile.wgsl). A pixel takes its ray from the visible voxel the raster kept at it, when
// that is a fresh voxel of a drawable entry of the node cache (pxl-engine `raster`,
// `record::fresh`; gpu/ray/raster.rs), as light.wgsl takes a slot's (visible.wgsl). The ray then
// carries what lies about the voxel's face. Otherwise the pixel may see the sky. Any other pixel
// is a hole and takes no walk here: it is listed for the queue, which ends its ray this frame
// (queue.wgsl), and its walked bit is set. A workgroup's square of pixels writes its walked words
// whole (walked.wgsl).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
/// The beam rays' words, with the holes' bits past the grid's words (walked.wgsl).
@group(0) @binding(8) var<storage, read_write> beams: array<u32>;
@group(0) @binding(9) var<storage, read_write> node_cache: array<u32>;

fn node_cache_word(k: u32) -> u32 {
  return node_cache[k];
}

/// Each drawn pixel's visible voxel (gpu/ray/raster.rs), as its number (`raster::id_of`), or 0
/// where the raster kept none. It is read as an array of one layer.
@group(0) @binding(13) var visibility: texture_2d_array<u32>;
@group(0) @binding(14) var<storage, read_write> args: MissCount;
/// The tiles claimed, after this pass's dispatch words: cast_tile.wgsl's `TileList`, its layout
/// read without the atomics, which a read-only binding cannot hold.
struct TileList {
  count: u32,
  sides: array<u32, 2>,
  cnt: u32,
  at: array<u32>,
}
@group(0) @binding(30) var<storage, read> claimed: TileList;

/// The first of the four samples about the screen's point at the tangents `at`
/// (`Grid::sample_before`).
fn sample_before(at: vec2<f32>) -> vec2<u32> {
  let v = vec2<f32>(at.x + eye_f32(TAN), eye_f32(TAN + 1u) - at.y);
  return vec2<u32>(max(floor(v * SAMPLE_SPACING_INVERSE), vec2<f32>(0.0))) + vec2<u32>(NEAR_SAMPLES);
}

/// The third word of the ray at the tangents `px` taken without a walk of its own, whose palette
/// indices are `ground`. In the ray steps view (pxl-engine `eye::STEPS`) it is the steps of the
/// grid's ray nearest it, the sample before it on the lattice: its beam ray's and its own
/// (`beam::SAMPLE_STEPS_SHIFT`).
fn taken_word(px: vec2<f32>, ground: u32) -> u32 {
  if (eye_word(STEPS) == 0u) {
    return ground;
  }
  return beam_steps(grid_cell(px)) + (beams[sample_word(sample_before(px)) + SAMPLE_LACKS] >> SAMPLE_STEPS_SHIFT);
}

fn write_ray(p: vec2<u32>, w: vec4<u32>) {
  for (var i = 0u; i < 4u; i += 1u) {
    textureStore(rays, p, i, vec4<u32>(w[i], 0u, 0u, 0u));
  }
}

/// The walked bits of the workgroup's square of pixels (`ray::SQUARE_WORDS`), as its pixels mark
/// them.
var<workgroup> marked: array<atomic<u32>, SQUARE_WORDS>;

/// Writes the ray of pixel `p` in a target of `size`, from its visible voxel or the sky. Returns
/// whether the pixel is a hole; a pixel past the target is not.
fn cast_px(p: vec2<u32>, size: vec2<u32>) -> bool {
  if (p.x >= size.x || p.y >= size.y) {
    return false;
  }
  let visible = textureLoad(visibility, vec2<i32>(p), 0i, 0i).x;
  let px = px_tan(p, size.xy);
  let aim = eye_aim(px);
  let on = ((node_cache[CNT_ON] >> CNT_EYE) & 1u) != 0u;
  if (visible != 0u && (visible & VISIBLE_LACK) == 0u) {
    // The voxel's entry was drawn, so no claim takes its slot while its box is drawn
    // (`record::drawn`). A slot forgotten since the boxes were drawn has no faces set to draw,
    // and the pixel walks.
    let slot = visible - 1u;
    let e = entry_at(slot);
    let state = node_cache_word(state_at(slot));
    if (entry_kind(slot) != KIND_SPLIT && (state & RECORD_ASKED) != 0u) {
      let k = entry_node(slot, vec3<i32>(aim.origin));
      let fill = (node_cache_word(e + W_ASSET) >> FILL_SHIFT) & (EIGHTHS - 1u);
      let faces = (state & RECORD_ASKED) >> RECORD_ASKED_SHIFT;
      let entry = visible_cell(aim, k.zoom, k.corner, k.roots - origin_roots(aim.origin), fill, faces);
      if (entry.enters && is_fresh(k, (state & RECORD_WHOLE) != 0u, vec3<i32>(aim.origin))) {
        // The ray's words, with what lies about its face taken from the voxel's (`word`,
        // `ray::first_word`).
        let face = 2u * entry.axis + select(0u, 1u, ray_dir(aim.dir)[entry.axis] > 0.0);
        add_cnt(on, CNT_VOXEL);
        write_ray(p, vec4<u32>(
          (node_cache_word(e + W_ASSET) & VAL_MASK) | (face << RAY_FACE_SHIFT) | (k.zoom << RAY_ZOOM_SHIFT) | (fill << RAY_FILL_SHIFT),
          bitcast<u32>(entry.t),
          taken_word(px, node_cache_word(e + W_ASSET + 1u)),
          node_cache_word(face_at(slot, face)),
        ));
        return false;
      }
    }
  }
  // With no fresh voxel, the pixel sees the sky when none of the four samples about it met
  // anything (`sky`). A thing thinner than the samples' spacing goes unseen.
  let s = sample_before(px);
  if (went_on(sample_word(s)) && went_on(sample_word(s + vec2<u32>(1u, 0u))) && went_on(sample_word(s + vec2<u32>(0u, 1u))) && went_on(sample_word(s + vec2<u32>(1u)))) {
    add_cnt(on, CNT_SKY);
    write_ray(p, vec4<u32>(0u, 0u, taken_word(px, 0u), 0u));
    return false;
  }
  add_cnt(on, CNT_HOLE);
  return true;
}

@compute @workgroup_size(CAST_SIDE, CAST_SIDE, 1)
fn cast_cs(@builtin(workgroup_id) w: vec3<u32>, @builtin(local_invocation_id) l: vec3<u32>) {
  let tile = claimed.at[w.x];
  let corner = vec2<u32>(tile & 0xffffu, tile >> 16u) * SCREEN_TILE + w.yz * CAST_SIDE;
  let size = textureDimensions(rays).xy;
  let p = corner + l.xy;
  if (cast_px(p, size)) {
    let b = hole_bit(p, size);
    atomicOr(&marked[b.x % SQUARE_WORDS], 1u << b.y);
    list_hole(eye_holes_at(size), p);
  }
  workgroupBarrier();
  // The square's walked words are written whole, a thread a word.
  let k = l.y * CAST_SIDE + l.x;
  if (k < SQUARE_WORDS && corner.x < size.x && corner.y < size.y) {
    beams[walked_at() + hole_bit(corner, size).x + k] = atomicLoad(&marked[k]);
  }
}
