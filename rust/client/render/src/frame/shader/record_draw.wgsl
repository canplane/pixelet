// pxl-render/record_draw.wgsl

// The record's draws, the last of its writers (pxl-engine record.rs). One thread a node cache
// slot writes a box for the raster for each drawable entry, an entry with a face set, and marks
// the entry drawn with the frame's number (`node_cache::R_DRAWN`), which keeps it from eviction
// (`record::drawn`).
//
// The eye's draw (`rec_draw_cs`) writes a box for each fresh drawable entry in view, which
// draws the faces an eye's ray asked about (pxl-engine `record::ASKED`). The draw records the
// frame it ran in (`node_cache::DRAWN`). Where the light's cast
// stands the world's receivers this frame (`light::HEADER_WORLD_RECEIVERS`), the draw also
// stands a receiver on each box it writes, by the rule every receiver stands by (light_box.wgsl
// `box_tiles`, light_receiver_tile.wgsl). A pixel that takes the box's voxel has its hit
// inside the box (visible.wgsl `visible_cell`), so the tiles the box stands on cover the hit.
//
// The light's draw (`light_rec_draw_cs`) writes a box for each drawable entry that lies on
// the light's box, in units from where the ray of box texel (0, 0) starts, as the raster lays
// the light's boxes. The box draws every face some ray entered (`record::FACES`). An entry
// whose node is not the one a ray asking its tile's band stops on is stale, and draws behind
// every fresh one (pxl-engine `record::light_fresh`).

/// The node cache, which the draws read and mark. Each thread marks only its own slot, so each
/// word has one writer.
@group(0) @binding(9) var<storage, read_write> node_cache: array<u32>;

fn node_cache_word(k: u32) -> u32 {
  return node_cache[k];
}

/// The boxes for the raster to draw, one word each (pxl-engine `raster::draw_word`).
@group(0) @binding(14) var<storage, read_write> draws: array<u32>;
/// The raster's indirect draw arguments, whose instances and count the draws raise (pxl-engine
/// `raster::ARGS_INSTANCES`, `raster::ARGS_CNT`).
@group(0) @binding(16) var<storage, read_write> rect_args: array<atomic<u32>>;

/// Writes box `place` for the entry in slot `slot`, as pxl-engine `raster::draw_word` lays it
/// out. The box is a lack's where `kind` is `KIND_LACKS`, and `flags` carries `RASTER_EYED`
/// and `RASTER_STALE`.
fn put_draw(place: u32, slot: u32, kind: u32, flags: u32) {
  draws[place] = (slot + 1u) | select(0u, RASTER_LACK, kind == KIND_LACKS) | flags;
}

/// Takes a place for one more box, or `NO_ENTRY` past `cap`. The count counts every box asked
/// for, past `cap` too, while the instances count only the boxes placed.
fn box_place(cap: u32) -> u32 {
  let place = atomicAdd(&rect_args[RASTER_ARGS_CNT], 1u);
  if (place >= cap) {
    return NO_ENTRY;
  }
  atomicMax(&rect_args[RASTER_ARGS_INSTANCES], place + 1u);
  return place;
}

/// Marks slot `slot` drawn in this frame, which keeps it from eviction (pxl-engine
/// `record::drawn`).
fn mark_drawn(slot: u32) {
  let frame = rec_frame();
  if (node_cache[drawn_at(slot)] != frame) {
    node_cache[drawn_at(slot)] = frame;
  }
}
/// Whether the box `lo`..`hi`, in units from the eye, meets what the eye sees (pxl-engine
/// `record::in_view`). Some point of it must lie before the eye, its nearest point along
/// `FORWARD` must be short of the eye's reach, and some point of it must lie inside each of the
/// screen's sides, widened by `REC_VIEW_GROWTH`.
fn in_view(lo: vec3<f32>, hi: vec3<f32>) -> bool {
  let f = eye_v3(FORWARD);
  let r = eye_v3(RIGHT);
  let u = eye_v3(UP);
  let tan = vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u)) * (1.0 + REC_VIEW_GROWTH);
  return reaches(f, lo, hi) > 0.0
    && -reaches(-f, lo, hi) < eye_f32(FAR)
    && reaches(tan.x * f - r, lo, hi) >= 0.0
    && reaches(tan.x * f + r, lo, hi) >= 0.0
    && reaches(tan.y * f - u, lo, hi) >= 0.0
    && reaches(tan.y * f + u, lo, hi) >= 0.0;
}

/// The greatest `n · p` over the points `p` of the box `lo`..`hi`.
fn reaches(n: vec3<f32>, lo: vec3<f32>, hi: vec3<f32>) -> f32 {
  let a = n * lo;
  let b = n * hi;
  let m = max(a, b);
  return m.x + m.y + m.z;
}

/// Whether the box `lo`..`hi` lies so near the eye that the raster's near plane may cut it,
/// given the tangents `tan_xy` of the screen's half-angles (pxl-engine raster.rs `is_eyed`).
fn is_eyed(lo: vec3<f32>, hi: vec3<f32>, tan_xy: vec2<f32>) -> bool {
  let v = select(select(vec3<f32>(0.0), hi, hi < vec3<f32>(0.0)), lo, lo > vec3<f32>(0.0));
  let k = 2.0 * RASTER_NEAR;
  return v.x * v.x + v.y * v.y + v.z * v.z < k * k * (1.0 + tan_xy.x * tan_xy.x + tan_xy.y * tan_xy.y);
}

@compute @workgroup_size(REC_DRAW_LANES, 1, 1)
fn rec_draw_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x == 0u) {
    node_cache[NODE_CACHE_DRAWN] = rec_frame();
  }
  if (id.x >= ENTRIES) {
    return;
  }
  let state = node_cache_word(state_at(id.x));
  // The faces an eye's ray entered and asked about (pxl-engine `record::ASKED`). An entry with
  // none does not draw for the eye.
  let faces = (state & REC_ASKED) >> REC_ASKED_SHIFT;
  if (faces == 0u) {
    return;
  }
  let kind = entry_kind(id.x);
  if (kind == KIND_SPLIT) {
    return;
  }
  let o = vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
  let k = entry_node(id.x, o);
  let b = asset_box(id.x, k, o);
  let tan_xy = vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u));
  let near = is_eyed(b.lo, b.hi, tan_xy);
  if (!near && !in_view(b.lo, b.hi)) {
    return;
  }
  // A stale voxel is no pixel's answer: a pixel only it covers walks as if no box stood there
  // (pxl-engine record.rs, "Fresh or stale"). Left undrawn, it stands no receiver and may be
  // evicted.
  if (!is_fresh(k, (state & REC_WHOLE) != 0u, o)) {
    return;
  }
  let place = box_place(REC_EYE_DRAWS);
  if (place == NO_ENTRY) {
    return;
  }
  mark_drawn(id.x);
  put_draw(place, id.x, kind, select(0u, RASTER_EYED, near));
  if (header_word(LIGHT_HEADER_WORLD_RECEIVERS) != 0u) {
    let t = box_tiles(b.lo, b.hi);
    if (t.has_tiles) {
      stand_box(t.first, t.last);
    }
  }
}

/// Where the point `v` lies on the light's box, in texels across and down from the middle of
/// texel (0, 0), as pxl-engine `Light::texel_at` finds it. `v` is in units from where the ray
/// of box texel (0, 0) starts.
fn box_texel(v: vec3<f32>) -> vec2<f32> {
  let across = dot(v, light_v3(LIGHT_ACROSS)) * eye_f32(LIGHT + LIGHT_ACROSS2_INVERSE);
  let down = dot(v, light_v3(LIGHT_DOWN)) * eye_f32(LIGHT + LIGHT_DOWN2_INVERSE);
  return vec2<f32>(across, down);
}

/// The tile of the light's box that the point `v` lies on, with `v` measured as in
/// `box_texel` (pxl-engine `Light::tile_of`).
fn box_tile(v: vec3<f32>) -> vec2<i32> {
  return vec2<i32>(floor((box_texel(v) + vec2<f32>(0.5)) * (1.0 / f32(LIGHT_TILE))));
}

/// Whether the box `lo`..`hi` lies on the light's box, that is, whether the tiles its corners
/// span meet the map's tiles.
fn on_box(lo: vec3<f32>, hi: vec3<f32>) -> bool {
  var first = vec2<i32>(2147483647i);
  var last = vec2<i32>(-2147483647i);
  for (var k = 0u; k < 8u; k += 1u) {
    let t = box_tile(select(lo, hi, vec3<bool>((k & 1u) != 0u, (k & 2u) != 0u, (k & 4u) != 0u)));
    first = min(first, t);
    last = max(last, t);
  }
  let n = i32(header_word(LIGHT_HEADER_TILES));
  return all(last >= vec2<i32>(0)) && all(first < vec2<i32>(n));
}

@compute @workgroup_size(REC_DRAW_LANES, 1, 1)
fn light_rec_draw_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= ENTRIES) {
    return;
  }
  let state = node_cache_word(state_at(id.x));
  let faces = state & REC_FACES;
  if (faces == 0u) {
    return;
  }
  let kind = entry_kind(id.x);
  if (kind == KIND_SPLIT) {
    return;
  }
  let o = texel_start(light_corner());
  let k = entry_node(id.x, o);
  let b = asset_box(id.x, k, o);
  if (!on_box(b.lo, b.hi)) {
    return;
  }
  let fresh = band_fresh(k.zoom, (state & REC_WHOLE) != 0u, tile_band(box_tile((b.lo + b.hi) * 0.5)));
  let place = box_place(REC_LIGHT_DRAWS);
  if (place == NO_ENTRY) {
    return;
  }
  mark_drawn(id.x);
  put_draw(place, id.x, kind, select(RASTER_STALE, 0u, fresh));
}
