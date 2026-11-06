// pxl-render/record_draw.wgsl

// The node cache over every slot, a thread a slot (pxl-engine record.rs): each drawable entry,
// a face set, drawn by the eye and by the light, and marked drawn in the frame's number
// (`node_cache::W_SEEN`; `record::drawn`). The eye's draw writes every drawable entry in view
// as a box for the raster's draw (`record_draw_cs`, raster.wgsl), its faces those an eye's ray
// asked about (pxl-engine `record::ASKED`), none where it has none, a stale one behind every
// fresh one (pxl-engine `raster::STALE`), a stale one about the eye not at all, and names the
// frame it ran (`node_cache::DRAWN`); and each box it writes stood on by the light's
// receivers, where they are stood this frame (`light::HEAD_STANDS_WORLD`), by the one rule
// every receiver stands by (light_box.wgsl `box_tiles`, light_stood.wgsl), as a pixel that
// takes the box's voxel hits within it (visible.wgsl `visible_cell`). The light's
// (`light_record_draw_cs`) writes every drawable entry whose box the light's box holds as a
// box for the light's raster, in units from where box texel (0, 0)'s ray starts, as the raster
// lays the light's boxes, its faces every one some ray entered (`record::FACES`), a stale one,
// not the node a ray asking its tile's band stops on, behind every fresh one (pxl-engine
// `record::light_fresh`).

/// The node cache, read and marked drawn: a thread a slot, so one writer a word.
@group(0) @binding(9) var<storage, read_write> node_cache: array<u32>;

fn node_cache_word(k: u32) -> u32 {
  return node_cache[k];
}

/// The raster's boxes (pxl-engine `raster::BOX_WORDS`).
@group(0) @binding(14) var<storage, read_write> boxes: array<u32>;
/// The raster's args (pxl-engine `raster::ARGS_INSTANCES`, `raster::ARGS_CNT`).
@group(0) @binding(16) var<storage, read_write> box_args: array<atomic<u32>>;

/// The entry at word `e`'s box from the origin `o`, the cell its asset keeps inset as it
/// fills (pxl-engine cast.rs `asset_box`).
fn entry_box(e: u32, k: Kept, o: vec3<i32>) -> Planes {
  let p = kept_planes(k, o);
  let eighths = (node_cache_word(e + W_ASSET) >> FILL_SHIFT) & (EIGHTHS - 1u);
  var lo = p.lo;
  var hi = p.hi;
  if (eighths != 0u) {
    let inset = (hi - lo) * (0.5 * (1.0 - side(eighths)));
    lo += inset;
    hi -= inset;
  }
  return Planes(lo, hi);
}

/// Box `place` of the draw's, the box `b` of slot `slot`, a lack where `kind` is, its faces
/// word `faces`.
fn put_box(place: u32, b: Planes, slot: u32, kind: u32, faces: u32) {
  let at = place * RASTER_BOX_WORDS;
  boxes[at + RASTER_BOX_LO] = bitcast<u32>(b.lo.x);
  boxes[at + RASTER_BOX_LO + 1u] = bitcast<u32>(b.lo.y);
  boxes[at + RASTER_BOX_LO + 2u] = bitcast<u32>(b.lo.z);
  boxes[at + RASTER_BOX_HI] = bitcast<u32>(b.hi.x);
  boxes[at + RASTER_BOX_HI + 1u] = bitcast<u32>(b.hi.y);
  boxes[at + RASTER_BOX_HI + 2u] = bitcast<u32>(b.hi.z);
  boxes[at + RASTER_BOX_ID] = (slot + 1u) | select(0u, RASTER_LACK, kind == KIND_LACKS);
  boxes[at + RASTER_BOX_FACES] = faces;
}

/// A place for a box among the draw's, `NO_ENTRY` past its room, its count raised.
fn box_place() -> u32 {
  let place = atomicAdd(&box_args[RASTER_ARGS_CNT], 1u);
  if (place >= RECORD_BOXES) {
    return NO_ENTRY;
  }
  atomicMax(&box_args[RASTER_ARGS_INSTANCES], place + 1u);
  return place;
}

/// The entry at word `e` marked drawn this frame (`record::drawn`).
fn mark_drawn(e: u32) {
  let frame = record_frame();
  if (node_cache[e + W_SEEN] != frame) {
    node_cache[e + W_SEEN] = frame;
  }
}
/// Whether the box `lo`..`hi`, units from the eye, meets what the eye sees: before it, within
/// its reach, and within the screen's sides grown by `RECORD_VIEW_GROWTH`
/// (pxl-engine `record::in_view`).
fn in_view(lo: vec3<f32>, hi: vec3<f32>) -> bool {
  let f = eye_v3(FORWARD);
  let r = eye_v3(RIGHT);
  let u = eye_v3(UP);
  let tan = vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u)) * (1.0 + RECORD_VIEW_GROWTH);
  return reaches(f, lo, hi) > 0.0
    && -reaches(-f, lo, hi) < eye_f32(FAR)
    && reaches(tan.x * f - r, lo, hi) >= 0.0
    && reaches(tan.x * f + r, lo, hi) >= 0.0
    && reaches(tan.y * f - u, lo, hi) >= 0.0
    && reaches(tan.y * f + u, lo, hi) >= 0.0;
}

/// The most of `n · p` over the box `lo`..`hi`.
fn reaches(n: vec3<f32>, lo: vec3<f32>, hi: vec3<f32>) -> f32 {
  let a = n * lo;
  let b = n * hi;
  let m = max(a, b);
  return m.x + m.y + m.z;
}

/// Whether the box lies so near the eye that the raster's near plane may cut it, `tan` the
/// screen's tangents (pxl-engine raster.rs `is_eyed`).
fn is_eyed(lo: vec3<f32>, hi: vec3<f32>, tan_xy: vec2<f32>) -> bool {
  let v = select(select(vec3<f32>(0.0), hi, hi < vec3<f32>(0.0)), lo, lo > vec3<f32>(0.0));
  let k = 2.0 * RASTER_NEAR;
  return v.x * v.x + v.y * v.y + v.z * v.z < k * k * (1.0 + tan_xy.x * tan_xy.x + tan_xy.y * tan_xy.y);
}

@compute @workgroup_size(RECORD_DRAW_LANES, 1, 1)
fn record_draw_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x == 0u) {
    node_cache[NODE_CACHE_DRAWN] = record_frame();
  }
  if (id.x >= ENTRIES) {
    return;
  }
  let e = entry_at(id.x);
  let kind = entry_kind(e);
  let state = node_cache_word(e + W_STATE);
  // The faces an eye's ray entered and asked what lies about (pxl-engine `record::ASKED`).
  let faces = (state & RECORD_ASKED) >> RECORD_ASKED_SHIFT;
  if (kind == KIND_SPLIT || faces == 0u) {
    return;
  }
  let o = vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
  let k = entry_kept(e, o);
  let b = entry_box(e, k, o);
  let tan_xy = vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u));
  let near = is_eyed(b.lo, b.hi, tan_xy);
  if (!near && !in_view(b.lo, b.hi)) {
    return;
  }
  let fresh = kept_fresh(k, (state & RECORD_WHOLE) != 0u, o);
  // A stale box about the eye is not drawn (pxl-engine `record`, Fresh or stale).
  if (near && !fresh) {
    return;
  }
  let place = box_place();
  if (place == NO_ENTRY) {
    return;
  }
  mark_drawn(e);
  put_box(place, b, id.x, kind, faces | select(0u, RASTER_EYED, near) | select(RASTER_STALE, 0u, fresh));
  if (head_word(LIGHT_HEAD_STANDS_WORLD) != 0u) {
    let t = box_tiles(b.lo, b.hi);
    if (t.got) {
      stand_box(t.first, t.last);
    }
  }
}

/// Where a point `v`, units from where box texel (0, 0)'s ray starts, lies on the box, in
/// texels across and down from texel (0, 0)'s middle (`Light::texel_at`).
fn box_texel(v: vec3<f32>) -> vec2<f32> {
  let across = dot(v, light_v3(LIGHT_ACROSS)) * eye_f32(LIGHT + LIGHT_ACROSS2_INVERSE);
  let down = dot(v, light_v3(LIGHT_DOWN)) * eye_f32(LIGHT + LIGHT_DOWN2_INVERSE);
  return vec2<f32>(across, down);
}

/// The tile a point `v` lies on, units as `box_texel` (`Light::tile_of`).
fn box_tile(v: vec3<f32>) -> vec2<i32> {
  return vec2<i32>(floor((box_texel(v) + vec2<f32>(0.5)) * (1.0 / f32(LIGHT_TILE))));
}

/// Whether the box `lo`..`hi` lies on the light's box: some tile its corners span is one of
/// the map's.
fn on_box(lo: vec3<f32>, hi: vec3<f32>) -> bool {
  var first = vec2<i32>(2147483647i);
  var last = vec2<i32>(-2147483647i);
  for (var k = 0u; k < 8u; k += 1u) {
    let t = box_tile(select(lo, hi, vec3<bool>((k & 1u) != 0u, (k & 2u) != 0u, (k & 4u) != 0u)));
    first = min(first, t);
    last = max(last, t);
  }
  let n = i32(head_word(LIGHT_HEAD_TILES));
  return all(last >= vec2<i32>(0)) && all(first < vec2<i32>(n));
}

@compute @workgroup_size(RECORD_DRAW_LANES, 1, 1)
fn light_record_draw_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= ENTRIES) {
    return;
  }
  let e = entry_at(id.x);
  let kind = entry_kind(e);
  let state = node_cache_word(e + W_STATE);
  let faces = state & RECORD_FACES;
  if (kind == KIND_SPLIT || faces == 0u) {
    return;
  }
  let o = texel_start(light_corner());
  let k = entry_kept(e, o);
  let b = entry_box(e, k, o);
  if (!on_box(b.lo, b.hi)) {
    return;
  }
  let fresh = band_fresh(k.zoom, (state & RECORD_WHOLE) != 0u, tile_band(box_tile((b.lo + b.hi) * 0.5)));
  let place = box_place();
  if (place == NO_ENTRY) {
    return;
  }
  mark_drawn(e);
  put_box(place, b, id.x, kind, faces | select(RASTER_STALE, 0u, fresh));
}
