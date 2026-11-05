// pxl-render/cast.wgsl

// Each drawn pixel's ray off the visible sample the raster kept at it (pxl-engine `raster`;
// gpu/ray/raster.rs), or the sky, a thread a pixel of the tiles claimed (cast_tiles.wgsl), as
// light.wgsl takes a slot's (visible.wgsl), with what lies about its face, the sample's. Any
// other pixel takes no walk here: its first word is the mark the walk again goes by
// (`ray::MISS`; miss_hit.wgsl), and it is listed for that walk (`node_cache::MARKS_AT`,
// list.wgsl `enlist`); the first past the list's room has the walk over every pixel
// dispatched instead.

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
@group(0) @binding(8) var<storage, read> beams: array<u32>;
@group(0) @binding(9) var<storage, read_write> node_cache: array<u32>;
/// Each drawn pixel's visible sample (gpu/ray/raster.rs): its number (`raster::id_of`), 0
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
  if (visible != 0u && (visible & VISIBLE_LACK) == 0u) {
    let n = eye_word(GRID + GRID_SAMPLES);
    let s = (visible & ~VISIBLE_LACK) - 1u;
    let k = sample_word(vec2<u32>(s % n, s / n));
    let entry = visible_entry(aim, k);
    if (entry.enters) {
      // Its words, with what lies about its face and the nodes its walk visited, the sample's
      // (`word`, `ray::first_word`).
      add_cnt(on, COUNT_REUSED);
      let asset = vec2<u32>(beams[k + SAMPLE_ASSET], beams[k + SAMPLE_ASSET + 1u]);
      let zoom = beams[k + SAMPLE_NODE + NODE_ZOOM];
      let face = 2u * entry.axis + select(0u, 1u, ray_dir(aim.dir)[entry.axis] > 0.0);
      let fill = (asset.x >> FILL_SHIFT) & (EIGHTHS - 1u);
      write_ray(p, vec4<u32>(
        (asset.x & VAL_MASK) | (face << RAY_FACE_SHIFT) | (zoom << RAY_ZOOM_SHIFT) | (fill << RAY_FILL_SHIFT),
        bitcast<u32>(entry.t),
        asset.y,
        beams[k + SAMPLE_FACES],
      ));
      return;
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
  // Neither: marked and listed for its walk (miss_hit.wgsl), which reads and writes the
  // node cache.
  add_cnt(on, COUNT_WALKED);
  textureStore(rays, p, 0u, vec4<u32>(RAY_MISS, 0u, 0u, 0u));
  let at = enlist(MARKS);
  if (at < MARKS) {
    node_cache[MARKS_AT + at] = (p.y << 16u) | p.x;
  } else if (at == MARKS) {
    // The one thread that finds the list full: the walk over every pixel dispatched instead.
    let whole = (size + vec2<u32>(CAST_SIDE - 1u)) / CAST_SIDE;
    args.whole[0] = whole.x;
    args.whole[1] = whole.y;
  }
}
