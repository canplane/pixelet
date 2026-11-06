// pxl-render/light.wgsl

// The world's depth from the light into every slot of the tiles the cast claimed (light_list.wgsl
// `light_item`): a workgroup a tile (`light::TILE`), a thread a slot, each taking the visible
// voxel, one of the light's record (pxl-engine `record`), which it marks seen, where its own
// ray enters it across a face some ray of the light entered it by, or the sky where
// no sample about it met anything, as cast.wgsl takes a pixel's (visible.wgsl), each only where
// that voxel was asked at the band the slot asks; the rest listed for the holes' queue
// (queue.wgsl).

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;
@group(0) @binding(10) var light: texture_storage_2d_array<r32uint, read_write>;
/// The visible voxel at each texel of the box, in the box's order (`Light::texel`): its
/// number (`raster::id_of`), 0 where it kept none.
@group(0) @binding(15) var visibility: texture_2d_array<u32>;
/// The listed slots' dispatch, their count and the counts' lanes (list.wgsl `MissArgs`).
@group(0) @binding(17) var<storage, read_write> args: MissArgs;

/// Whether sample `s` was walked at `band`, the band of box texel `p`'s tile: the band of the
/// tile it lies on (light_aim.wgsl `tile_band`, pxl-engine `Light::band`), that tile's own
/// where it lies on `p`'s.
fn asks_alike(s: vec2<u32>, band: u32, p: vec2<u32>) -> bool {
  let on = light_on(LEVEL_RAY, s * LIGHT_SAMPLE);
  return all(on == vec2<i32>(p / LIGHT_TILE)) || tile_band(on) == band;
}

/// Whether sample `s` went on to its reach, asked at `band` (`beam::SAMPLE_NONE`).
fn went_on_alike(s: vec2<u32>, band: u32, p: vec2<u32>) -> bool {
  return went_on(light_sample_at(s)) && asks_alike(s, band, p);
}

/// Whether the four samples about slot `at`, box texel `p`, each went on to their reach,
/// asked at `band`; none where the four lie not about it, within a sample of the box's edge
/// (pxl-engine `light::samples_about`).
fn sky(at: vec2<u32>, p: vec2<u32>, band: u32) -> bool {
  let d = LIGHT_SAMPLE;
  let n = head_word(LIGHT_HEAD_SIZE);
  let r = at % d;
  let about = p.x >= r.x && p.x - r.x + d < n && p.y >= r.y && p.y - r.y + d < n;
  let s = at / d;
  return about
    && went_on_alike(s, band, p)
    && went_on_alike(s + vec2<u32>(1u, 0u), band, p)
    && went_on_alike(s + vec2<u32>(0u, 1u), band, p)
    && went_on_alike(s + vec2<u32>(1u, 1u), band, p);
}

/// Each slot of the tiles the cast claimed (light_list.wgsl `light_item`): its word as it stands
/// where the cast before answered it and its tile was carried over (`ray::KEPT`); else off its
/// visible voxel or the sky, else listed for its walk as the eye's missed pixels are. The list
/// holds every slot, so none finds it full.
@compute @workgroup_size(LIGHT_TILE, LIGHT_TILE, 1)
fn light_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let size = head_word(LIGHT_HEAD_SIZE);
  let at = light_item(LEVEL_RAY, group.xy * LIGHT_TILE + lane.xy);
  if (at.x >= size || at.y >= size) {
    return;
  }
  let on = head_word(LIGHT_HEAD_COUNTED) != 0u;
  let k = (at.y % LIGHT_TILE) * LIGHT_TILE + at.x % LIGHT_TILE;
  let state = light_state_at(beams[light_place_at(at)], k, head_word(LIGHT_HEAD_CAST));
  if ((beams[state + STATE_WORDS - 1u] & KEPT) != 0u) {
    add_cnt(on, COUNT_REUSED);
    return;
  }
  let p = slot_texel(at);
  let aim = light_aim(p);
  let band = aim.range.x;
  let visible = textureLoad(visibility, p, 0u, 0u).x;
  if (visible != 0u) {
    // A voxel of the light's record (pxl-engine `record`), seen this cast: no claim takes its
    // slot this cast or the next (`record::victim`).
    let e = record_at((visible & ~VISIBLE_LACK) - 1u);
    let frame = record_frame();
    if (record_word(e + R_SEEN) != frame) {
      atomicStore(&record[e + R_SEEN], frame);
    }
    let held = record_word(e + R_HELD);
    if ((visible & VISIBLE_LACK) == 0u && (held & RECORD_HELD) != 0u) {
      let k = entry_kept(e, held);
      let fill = (record_word(e + R_ASSET) >> FILL_SHIFT) & (EIGHTHS - 1u);
      let faces = record_word(e + R_STATE) & RECORD_FACES;
      let entry = visible_cell(aim, k.zoom, k.corner, k.roots - origin_roots(aim.origin), fill, faces);
      // Taken only where the voxel was asked at the slot's band (pxl-engine `Light::band`).
      if (entry.enters && k.band == band) {
        add_cnt(on, COUNT_REUSED);
        textureStore(light, at, 0u, vec4<u32>(stop_word(entry.t, entry.axis), 0u, 0u, 0u));
        return;
      }
    }
  }
  if (visible == 0u && sky(at, p, band)) {
    add_cnt(on, COUNT_SKY);
    textureStore(light, at, 0u, vec4<u32>(depth_word(1.0), 0u, 0u, 0u));
    return;
  }
  add_cnt(on, COUNT_WALKED);
  let place = enlist(size * size);
  if (place < size * size) {
    beams[head_word(LIGHT_HEAD_MISSES) + place] = (at.y << 16u) | at.x;
  }
}
