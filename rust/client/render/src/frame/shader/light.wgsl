// pxl-render/light.wgsl

// Writes the world's depth from the light into every slot of the tiles the cast claimed
// (light_list.wgsl `light_item`), one workgroup for each tile (`light::TILE`) and one thread
// for each slot. A slot takes its visible voxel, a drawable entry of the node cache
// (pxl-engine `record`), where its own ray enters the voxel across a face some ray entered it
// by. It takes the sky where no sample about it met anything. cast.wgsl takes a pixel's ray
// the same way (visible.wgsl). A slot takes its voxel only where that voxel is the node a ray
// asking the slot's band stops on. The other slots are listed for the holes' queue
// (queue.wgsl).

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;
@group(0) @binding(9) var<storage, read> node_cache: array<u32>;

fn node_cache_word(k: u32) -> u32 {
  return node_cache[k];
}

@group(0) @binding(10) var light: texture_storage_2d_array<r32uint, read_write>;
/// The visible voxel at each texel of the box, in the box's order (`Light::texel`), as its
/// number (`raster::id_of`). A texel that kept no voxel holds 0.
@group(0) @binding(15) var visibility: texture_2d_array<u32>;
@group(0) @binding(17) var<storage, read_write> args: MissCount;

/// Whether sample `s` was walked at `band`, the band of the tile of box texel `p`. A sample is
/// walked at the band of the tile it lies on (light_aim.wgsl `tile_band`, pxl-engine
/// `Light::band`), which is `band` itself where that tile is `p`'s.
fn asks_alike(s: vec2<u32>, band: u32, p: vec2<u32>) -> bool {
  let on = light_on(LEVEL_RAY, s * LIGHT_SAMPLE);
  return all(on == vec2<i32>(p / LIGHT_TILE)) || tile_band(on) == band;
}

/// Whether sample `s` went on to its reach while asking at `band` (`beam::SAMPLE_NONE`).
fn went_on_alike(s: vec2<u32>, band: u32, p: vec2<u32>) -> bool {
  return went_on(light_sample_at(s)) && asks_alike(s, band, p);
}

/// Whether the four samples about slot `at`, box texel `p`, each went on to its reach while
/// asking at `band`. Within a sample of the box's edge the four do not lie about the texel, and
/// the slot takes no sky (pxl-engine `light::samples_about`).
fn sky(at: vec2<u32>, p: vec2<u32>, band: u32) -> bool {
  let d = LIGHT_SAMPLE;
  let n = header_word(LIGHT_HEADER_SIZE);
  let r = at % d;
  let about = p.x >= r.x && p.x - r.x + d < n && p.y >= r.y && p.y - r.y + d < n;
  let s = at / d;
  return about
    && went_on_alike(s, band, p)
    && went_on_alike(s + vec2<u32>(1u, 0u), band, p)
    && went_on_alike(s + vec2<u32>(0u, 1u), band, p)
    && went_on_alike(s + vec2<u32>(1u, 1u), band, p);
}

/// Each slot of the tiles the cast claimed (light_list.wgsl `light_item`) takes its visible
/// voxel or the sky, or else is listed as a hole, as the eye's pixels are. The list has room for
/// every slot of the map (pxl-engine `light::holes_at`), so no slot finds it full.
@compute @workgroup_size(LIGHT_TILE, LIGHT_TILE, 1)
fn light_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let size = header_word(LIGHT_HEADER_SIZE);
  let at = light_item(LEVEL_RAY, group.xy * LIGHT_TILE + lane.xy);
  if (at.x >= size || at.y >= size) {
    return;
  }
  let on = header_word(LIGHT_HEADER_COUNTED) != 0u;
  let p = slot_texel(at);
  let aim = light_aim(p);
  let band = aim.range.x;
  let visible = textureLoad(visibility, p, 0u, 0u).x;
  if (visible != 0u) {
    // The visible voxel is a drawable entry drawn this cast, so no walk claims its slot while
    // this pass reads it (`record::drawn`).
    let slot = (visible & ~VISIBLE_LACK) - 1u;
    let state = node_cache_word(state_at(slot));
    let faces = state & RECORD_FACES;
    if ((visible & VISIBLE_LACK) == 0u && entry_kind(slot) != KIND_SPLIT && faces != 0u) {
      let k = entry_node(slot, vec3<i32>(aim.origin));
      let fill = (node_cache_word(entry_at(slot) + W_ASSET) >> FILL_SHIFT) & (EIGHTHS - 1u);
      let entry = visible_cell(aim, k.zoom, k.corner, k.roots - origin_roots(aim.origin), fill, faces);
      // The voxel is taken only where it is the node that a ray asking the slot's band stops on
      // (pxl-engine `Light::band`, `record::band_fresh`).
      if (entry.enters && band_fresh(k.zoom, (state & RECORD_WHOLE) != 0u, band)) {
        add_cnt(on, COUNT_VOXEL);
        textureStore(light, at, 0u, vec4<u32>(hit_word(entry.t, entry.axis), 0u, 0u, 0u));
        return;
      }
    }
  }
  if (visible == 0u && sky(at, p, band)) {
    add_cnt(on, COUNT_SKY);
    textureStore(light, at, 0u, vec4<u32>(depth_word(1.0), 0u, 0u, 0u));
    return;
  }
  add_cnt(on, COUNT_HOLE);
  list_hole(header_word(LIGHT_HEADER_HOLES), at);
}
