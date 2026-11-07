// pxl-render/cast.wgsl

// Each drawn pixel's ray off the visible voxel the raster kept at it, a fresh voxel of a
// drawable entry of the node cache (pxl-engine `raster`, `record::fresh`; gpu/ray/raster.rs),
// or the sky, a thread a pixel of the tiles claimed (cast_tiles.wgsl), as light.wgsl takes a
// slot's (visible.wgsl), with what lies about its face, the voxel's. Any other pixel, a hole,
// takes no walk here: its walked and owed bits are set for the holes' queue (walked.wgsl;
// queue.wgsl), a workgroup's square of pixels writing its words whole, and its words are what
// was drawn there until the queue ends its ray: the visible voxel, fresh or stale, at its box's
// entry; where no entry draws, the words the pixel holds, the last answer it showed, are left
// as they are, and the zero of a ray that met nothing where it has shown none. So the pixel
// never waits on its walk, and a ray not ended overwrites nothing.

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
/// The walk again's words (list.wgsl `MissArgs`).
@group(0) @binding(14) var<storage, read_write> args: MissArgs;
/// The tiles claimed, this pass's dispatch ahead of them (cast_tiles.wgsl).
struct CastTiles {
  args: array<u32, 3>,
  cnt: u32,
  at: array<u32>,
}
@group(0) @binding(30) var<storage, read> claimed: CastTiles;

/// The first of the four samples about the screen's point at the tangents `at`
/// at the offset the samples were cast at (`Grid::sample_before`, `node_cache::ROLL_AT`).
fn sample_before(at: vec2<f32>) -> vec2<u32> {
  let roll = vec2<f32>(bitcast<f32>(node_cache_word(ROLL_AT)), bitcast<f32>(node_cache_word(ROLL_AT + 1u)));
  let v = vec2<f32>(at.x + eye_f32(TAN) - roll.x, eye_f32(TAN + 1u) - at.y - roll.y);
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
  // What was drawn at the pixel, its words where it is a hole.
  var drew = vec4<u32>(0u);
  if (visible != 0u) {
    // The voxel's entry, drawn and so kept from any claim while its box is drawn
    // (`record::drawn`). A slot forgotten since the boxes were drawn draws none: the pixel
    // walks.
    let e = entry_at((visible & ~VISIBLE_LACK) - 1u);
    let state = node_cache_word(e + W_STATE);
    if (entry_kind(e) != KIND_SPLIT && (state & RECORD_ASKED) != 0u) {
      let k = entry_kept(e, vec3<i32>(aim.origin));
      let asset = vec2<u32>(node_cache_word(e + W_ASSET), node_cache_word(e + W_ASSET + 1u));
      let fill = (asset.x >> FILL_SHIFT) & (EIGHTHS - 1u);
      let faces = (state & RECORD_ASKED) >> RECORD_ASKED_SHIFT;
      let fresh = kept_fresh(k, (state & RECORD_WHOLE) != 0u, vec3<i32>(aim.origin));
      let entry = visible_cell(aim, k.zoom, k.corner, k.roots - origin_roots(aim.origin), fill, faces);
      // Its words, with what lies about its face, the voxel's (`word`, `ray::first_word`).
      let face = 2u * entry.axis + select(0u, 1u, ray_dir(aim.dir)[entry.axis] > 0.0);
      drew = vec4<u32>(
        (asset.x & VAL_MASK) | (face << RAY_FACE_SHIFT) | (k.zoom << RAY_ZOOM_SHIFT) | (fill << RAY_FILL_SHIFT),
        bitcast<u32>(entry.t),
        asset.y,
        node_cache_word(e + W_FACES + face),
      );
      if ((visible & VISIBLE_LACK) == 0u && entry.enters && fresh) {
        add_cnt(on, COUNT_REUSED);
        write_ray(p, drew);
        return false;
      }
    }
  }
  if (visible == 0u) {
    // The sky where none of the four samples about it met anything (`sky`), taken without a
    // walk of its own, as a thing thinner than their spacing goes unseen.
    let s = sample_before(px);
    let k = sample_word(s);
    if (went_on(k) && went_on(sample_word(s + vec2<u32>(1u, 0u))) && went_on(sample_word(s + vec2<u32>(0u, 1u))) && went_on(sample_word(s + vec2<u32>(1u)))) {
      add_cnt(on, COUNT_SKY);
      write_ray(p, vec4<u32>(0u));
      return false;
    }
  }
  // Neither: what was drawn, and marked for the holes' queue (queue.wgsl), which reads and
  // writes the node cache.
  add_cnt(on, COUNT_WALKED);
  if (any(drew != vec4<u32>(0u))) {
    write_ray(p, drew);
  }
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
  }
  workgroupBarrier();
  // The square's words whole, a thread a word, walked and owed alike: the cast's run takes its
  // holes as the cast marked them.
  let k = l.y * CAST_SIDE + l.x;
  if (k < SQUARE_WORDS && corner.x < size.x && corner.y < size.y) {
    let at = walked_at() + hole_bit(corner, size).x + k;
    let owed = at + hole_words(size);
    if (owed < arrayLength(&beams)) {
      let bits = atomicLoad(&marked[k]);
      beams[at] = bits;
      beams[owed] = bits;
    }
  }
}
