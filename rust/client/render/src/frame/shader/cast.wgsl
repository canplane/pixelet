// pxl-render/cast.wgsl

// Each drawn pixel's ray off the visible voxel the raster kept at it, a voxel of the eye's
// record, which it marks seen (pxl-engine `raster`, `record`; gpu/ray/raster.rs), or the
// sky, a thread a pixel of the tiles claimed (cast_tiles.wgsl), as light.wgsl takes a slot's
// (visible.wgsl), with what lies about its face, the voxel's. Any other pixel takes no walk
// here: its first word is the mark the holes' queue goes by (`ray::MISS`; queue.wgsl), and
// it is listed for that walk (walked.wgsl, list.wgsl `enlist`), in a list with room for every
// pixel drawn, each listed once a cast.

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
/// The beam rays' words, the walked pixels' list past the grid's (walked.wgsl).
@group(0) @binding(8) var<storage, read_write> beams: array<u32>;
@group(0) @binding(9) var<storage, read_write> node_cache: array<u32>;
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
  let visible = textureLoad(visibility, vec2<i32>(p), 0i, 0i).x;
  let on = node_cache[COUNT_ON] != 0u;
  if (visible != 0u) {
    // Seen this frame: no sample's claim takes its slot this frame or the next
    // (`record::victim`).
    let e = record_at((visible & ~VISIBLE_LACK) - 1u);
    let frame = record_frame();
    if (record_word(e + R_SEEN) != frame) {
      atomicStore(&record[e + R_SEEN], frame);
    }
    // A slot forgotten since the boxes were drawn holds no voxel: the pixel walks.
    let held = record_word(e + R_HELD);
    if ((visible & VISIBLE_LACK) == 0u && (held & RECORD_HELD) != 0u) {
      let k = entry_kept(e, held);
      let asset = vec2<u32>(record_word(e + R_ASSET), record_word(e + R_ASSET + 1u));
      let fill = (asset.x >> FILL_SHIFT) & (EIGHTHS - 1u);
      let faces = record_word(e + R_STATE) & RECORD_FACES;
      let entry = visible_cell(aim, k.zoom, k.corner, k.roots - origin_roots(aim.origin), fill, faces);
      if (entry.enters) {
        // Its words, with what lies about its face, the voxel's (`word`, `ray::first_word`).
        add_cnt(on, COUNT_REUSED);
        let face = 2u * entry.axis + select(0u, 1u, ray_dir(aim.dir)[entry.axis] > 0.0);
        write_ray(p, vec4<u32>(
          (asset.x & VAL_MASK) | (face << RAY_FACE_SHIFT) | (k.zoom << RAY_ZOOM_SHIFT) | (fill << RAY_FILL_SHIFT),
          bitcast<u32>(entry.t),
          asset.y,
          record_word(e + R_FACES + entry.axis),
        ));
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
      return;
    }
  }
  // Neither: marked and listed for the holes' queue (queue.wgsl), which reads and writes the
  // node cache. The list holds a place for every pixel drawn, and a pixel is listed once.
  add_cnt(on, COUNT_WALKED);
  textureStore(rays, p, 0u, vec4<u32>(RAY_MISS, 0u, 0u, 0u));
  let drawn = size.x * size.y;
  let at = enlist(drawn);
  let place = walked_list_at(at);
  if (at < drawn && place < arrayLength(&beams)) {
    beams[place] = (p.y << 16u) | p.x;
  }
}
