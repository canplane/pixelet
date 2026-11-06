// pxl-render/cast.wgsl

// Each drawn pixel's ray off the visible voxel the raster kept at it, a fresh voxel of a
// drawable entry of the node cache (pxl-engine `raster`, `record::fresh`; gpu/ray/raster.rs),
// or the sky, a thread a pixel of the tiles claimed (cast_tiles.wgsl), as light.wgsl takes a
// slot's (visible.wgsl), with what lies about its face, the voxel's. Any other pixel, a hole,
// takes no walk here: it is listed for the holes' queue (walked.wgsl, list.wgsl `enlist`;
// queue.wgsl), in a list with room for every pixel drawn, each listed once a cast, and its
// words are what was drawn there until the queue ends its ray: the visible voxel, fresh or
// stale, at its box's entry, or the zero of a ray that met nothing. So the pixel never waits
// on its walk, and shows nothing no entry draws. Every pixel the cast takes is set to no
// parked ray, but a hole where the rays stand (`node_cache::RESUMES`), whose parked state the
// queue goes on from.

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
/// The beam rays' words, the walked pixels' list past the grid's (walked.wgsl).
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

/// Pixel `p`'s parked state, of a target `size`, set to no parked ray.
fn unpark_px(p: vec2<u32>, size: vec2<u32>) {
  let at = eye_state_at(p, size, arrayLength(&beams)) + STATE_WORDS - 1u;
  if (at < arrayLength(&beams)) {
    beams[at] = 0u;
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
        unpark_px(p, size.xy);
        return;
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
      unpark_px(p, size.xy);
      return;
    }
  }
  // Neither: what was drawn, and listed for the holes' queue (queue.wgsl), which reads
  // and writes the node cache. The list holds a place for every pixel drawn, and a pixel is
  // listed once.
  add_cnt(on, COUNT_WALKED);
  write_ray(p, drew);
  if (node_cache[RESUMES] == 0u) {
    unpark_px(p, size.xy);
  }
  let drawn = size.x * size.y;
  let at = enlist(drawn);
  let place = walked_list_at(at);
  if (at < drawn && place < arrayLength(&beams)) {
    beams[place] = (p.y << 16u) | p.x;
  }
}
