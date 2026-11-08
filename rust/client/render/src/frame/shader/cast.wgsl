// pxl-render/cast.wgsl

// Each drawn pixel's ray off the visible voxel the raster kept at it, a fresh voxel of a
// drawable entry of the node cache (pxl-engine `raster`, `record::fresh`; gpu/ray/raster.rs),
// or the sky, a thread a pixel of the tiles claimed (cast_tiles.wgsl), as light.wgsl takes a
// slot's (visible.wgsl), with what lies about its face, the voxel's. Any other pixel, a hole,
// takes no walk here: it is listed for the queue, which ends its ray this frame (queue.wgsl),
// and its walked bit set, a workgroup's square of pixels writing its words whole (walked.wgsl).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
/// The beam rays' words, the holes' bits past the grid's (walked.wgsl).
@group(0) @binding(8) var<storage, read_write> beams: array<u32>;
@group(0) @binding(9) var<storage, read_write> node_cache: array<u32>;

fn node_cache_word(k: u32) -> u32 {
  return node_cache[k];
}

/// Each drawn pixel's visible voxel (gpu/ray/raster.rs): its number (`raster::id_of`), 0
/// where it kept none, read as an array of one layer.
@group(0) @binding(13) var visibility: texture_2d_array<u32>;
/// The holes' count and the counts' lanes (visible.wgsl `MissArgs`).
@group(0) @binding(14) var<storage, read_write> args: MissArgs;
/// The tiles claimed, this pass's dispatch ahead of them: cast_tiles.wgsl's `CastTiles`, its
/// layout read without the atomics a read-only binding cannot hold.
struct CastTiles {
  count: u32,
  sides: array<u32, 2>,
  cnt: u32,
  at: array<u32>,
}
@group(0) @binding(30) var<storage, read> claimed: CastTiles;

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

/// The workgroup's square's walked bits (`ray::SQUARE_WORDS`), as its pixels mark them.
var<workgroup> marked: array<atomic<u32>, SQUARE_WORDS>;

/// Pixel `p`'s ray, of a target `size`, off its visible voxel or the sky; whether it is a hole,
/// none where it lies past the target.
fn cast_px(p: vec2<u32>, size: vec2<u32>) -> bool {
  if (p.x >= size.x || p.y >= size.y) {
    return false;
  }
  let visible = textureLoad(visibility, vec2<i32>(p), 0i, 0i).x;
  let px = px_tan(p, size.xy);
  let aim = eye_aim(px);
  let on = ((node_cache[COUNT_ON] >> COUNT_EYE) & 1u) != 0u;
  if (visible != 0u && (visible & VISIBLE_LACK) == 0u) {
    // The voxel's entry, drawn and so kept from any claim while its box is drawn
    // (`record::drawn`). A slot forgotten since the boxes were drawn draws none: the pixel
    // walks.
    let slot = visible - 1u;
    let e = entry_at(slot);
    let state = node_cache_word(state_at(slot));
    if (entry_kind(slot) != KIND_SPLIT && (state & RECORD_ASKED) != 0u) {
      let k = entry_node(slot, vec3<i32>(aim.origin));
      let fill = (node_cache_word(e + W_ASSET) >> FILL_SHIFT) & (EIGHTHS - 1u);
      let faces = (state & RECORD_ASKED) >> RECORD_ASKED_SHIFT;
      let entry = visible_cell(aim, k.zoom, k.corner, k.roots - origin_roots(aim.origin), fill, faces);
      if (entry.enters && kept_fresh(k, (state & RECORD_WHOLE) != 0u, vec3<i32>(aim.origin))) {
        // Its words, with what lies about its face, the voxel's (`word`, `ray::first_word`).
        let face = 2u * entry.axis + select(0u, 1u, ray_dir(aim.dir)[entry.axis] > 0.0);
        add_cnt(on, COUNT_REUSED);
        write_ray(p, vec4<u32>(
          (node_cache_word(e + W_ASSET) & VAL_MASK) | (face << RAY_FACE_SHIFT) | (k.zoom << RAY_ZOOM_SHIFT) | (fill << RAY_FILL_SHIFT),
          bitcast<u32>(entry.t),
          node_cache_word(e + W_ASSET + 1u),
          node_cache_word(face_at(slot, face)),
        ));
        return false;
      }
    }
  }
  // No fresh voxel: the sky where none of the four samples about it met anything (`sky`), as a
  // thing thinner than their spacing goes unseen.
  let s = sample_before(px);
  if (went_on(sample_word(s)) && went_on(sample_word(s + vec2<u32>(1u, 0u))) && went_on(sample_word(s + vec2<u32>(0u, 1u))) && went_on(sample_word(s + vec2<u32>(1u)))) {
    add_cnt(on, COUNT_SKY);
    write_ray(p, vec4<u32>(0u));
    return false;
  }
  add_cnt(on, COUNT_WALKED);
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
  // The square's words whole, a thread a word.
  let k = l.y * CAST_SIDE + l.x;
  if (k < SQUARE_WORDS && corner.x < size.x && corner.y < size.y) {
    beams[walked_at() + hole_bit(corner, size).x + k] = atomicLoad(&marked[k]);
  }
}
