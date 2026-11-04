// pxl-render/light.wgsl

// The world's depth from the light into every slot the cast needs (pxl-shader observer.rs
// `resolve`): a workgroup a tile (`light::TILE`), a thread a slot, each taking the visible
// sample where its own ray enters it as that sample entered it (`took`), or the sky where no
// sample about it met anything (`sky`), each only where that sample asked the band the slot
// asks; the rest listed for their walk (light_miss_hit.wgsl, light_miss.wgsl).

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;
@group(0) @binding(10) var light: texture_storage_2d_array<r32uint, read_write>;
/// The visible sample at each texel of the box, in the box's order (`Light::texel`): its
/// sample's number (`raster::id_of`), 0 where it kept none.
@group(0) @binding(15) var visibility: texture_2d_array<u32>;
/// The listed slots' dispatch, their count and the tally's lanes (`ray::MISS_ARGS_WORDS`).
@group(0) @binding(17) var<storage, read_write> misses: array<atomic<u32>>;

/// One more of the tally's lane `lane` (`tally::TOOK` on) where the cast counts (cast.rs
/// `add_cnt`).
fn add_cnt(lane: u32) {
  if (head_word(LIGHT_HEAD_COUNTED) != 0u) {
    atomicAdd(&misses[MISS_ARGS_TALLY + lane - TALLY_TOOK], 1u);
  }
}

/// Whether sample `s` was walked at `band` (observer.rs `asks_alike`).
fn asks_alike(s: vec2<u32>, band: u32) -> bool {
  return tile_band(light_on(LEVEL_RAY, s * LIGHT_SAMPLE)) == band;
}

/// Whether sample `s` went on to its reach, asked at `band` (observer.rs `went_on`).
fn went_on(s: vec2<u32>, band: u32) -> bool {
  return beams[light_sample_at(s) + SAMPLE_NODE + NODE_ZOOM] == SAMPLE_NONE && asks_alike(s, band);
}

/// Whether the four samples about slot `at` each went on to their reach, asked at `band`;
/// none where the four lie not about it, within a sample of the box's edge (observer.rs
/// `sky`, `Map::sample_before`).
fn sky(at: vec2<u32>, p: vec2<u32>, band: u32) -> bool {
  let d = LIGHT_SAMPLE;
  let n = head_word(LIGHT_HEAD_SIZE);
  let r = at % d;
  let about = p.x >= r.x && p.x - r.x + d < n && p.y >= r.y && p.y - r.y + d < n;
  let s = at / d;
  return about
    && went_on(s, band)
    && went_on(s + vec2<u32>(1u, 0u), band)
    && went_on(s + vec2<u32>(0u, 1u), band)
    && went_on(s + vec2<u32>(1u, 1u), band);
}

/// Where a slot's ray takes its visible sample, and whether it does.
struct Took {
  t: f32,
  axis: u32,
  enters: bool,
}

/// The cell the ray from `origin` takes off the visible sample `visible`, one that lacks
/// not: where it enters that sample's cell as drawn (`cast::entry`, `fill_bounds`), taken
/// where it enters it across the axis the sample did, asked at its band (observer.rs `took`,
/// raster.rs `visible_cell`).
fn took(origin: vec3<i32>, visible: u32, band: u32) -> Took {
  let n = head_word(LIGHT_HEAD_SIZE) / LIGHT_SAMPLE;
  let id = (visible & ~VISIBLE_LACK) - 1u;
  let s = vec2<u32>(id % n, id / n);
  let k = light_sample_at(s) + SAMPLE_NODE;
  let corner = vec3<u32>(beams[k + NODE_CORNER], beams[k + NODE_CORNER + 1u], beams[k + NODE_CORNER + 2u]);
  let roots = vec3<i32>(i32(beams[k + NODE_ROOTS]), i32(beams[k + NODE_ROOTS + 1u]), i32(beams[k + NODE_ROOTS + 2u]));
  let zoom = beams[k + NODE_ZOOM];
  let asset = beams[light_sample_at(s) + SAMPLE_ASSET];
  let og = vec3<u32>(origin) + vec3<u32>(0u, Y_START, Z_START);
  let lo = units(corner, og, roots);
  let hi = far_of(corner, under(zoom), og, roots);
  let inset = (hi - lo) * (0.5 * (1.0 - side((asset >> FILL_SHIFT) & (EIGHTHS - 1u))));
  let dir = ray_dir(light_v3(LIGHT_DIR));
  let t_max = eye_f32(LIGHT + LIGHT_T_MAX);
  let b = span_of(dir, ray_inv(dir), lo + inset, hi - inset, 0.0, t_max);
  let t0 = max(0.0, largest(b.near));
  let t1 = min(t_max, smallest(b.far));
  let axis = crossing(dir, b.near, true);
  let enters = t1 > t0 && axis == beams[light_sample_at(s) + SAMPLE_AXIS] && asks_alike(s, band);
  return Took(max(b.near[axis], 0.0), axis, enters);
}

/// Each slot of the tiles the cast casts (`Map::needs`): its word off its visible sample or
/// the sky, else listed for its walk as the eye's missed pixels are (cast.rs `enlist`). The
/// list holds every slot, so none finds it full.
@compute @workgroup_size(8, 8, 1)
fn light_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let size = head_word(LIGHT_HEAD_SIZE);
  let at = light_take(LEVEL_RAY, group.xy * LIGHT_TILE + lane.xy);
  if (at.x >= size || at.y >= size || !light_needs(LEVEL_RAY, at)) {
    return;
  }
  let p = slot_texel(at);
  let origin = texel_start(light_corner() + vec2<i32>(p));
  let band = texel_band(p);
  let visible = textureLoad(visibility, p, 0u, 0u).x;
  if (visible != 0u && (visible & VISIBLE_LACK) == 0u) {
    let hit = took(origin, visible, band);
    if (hit.enters) {
      add_cnt(TALLY_TOOK);
      textureStore(light, at, 0u, vec4<u32>(stop_word(hit.t, hit.axis), 0u, 0u, 0u));
      return;
    }
  }
  if (visible == 0u && sky(at, p, band)) {
    add_cnt(TALLY_SKY);
    textureStore(light, at, 0u, vec4<u32>(depth_word(1.0), 0u, 0u, 0u));
    return;
  }
  add_cnt(TALLY_WALKED);
  let place = atomicAdd(&misses[MISS_ARGS_CNT], 1u);
  if (place < size * size) {
    // The rows the list fills raised to hold it by the place that opens each (`opens`).
    let per = MISS_LANES * MISS_ACROSS;
    if (place % per == 0u) {
      atomicMax(&misses[MISS_ARGS_LIST + 1u], place / per + 1u);
    }
    beams[head_word(LIGHT_HEAD_MISSES) + 1u + place] = (at.y << 16u) | at.x;
  }
}
