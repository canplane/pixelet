// pxl-render/record_draw.wgsl

// The eye's record over every slot, a thread a slot (pxl-engine record.rs): forgotten where
// the node cache's rule forgets (`record_forget_cs`, forget.wgsl), and drawn, a key one entry
// (`joins_first`), every held voxel in view written as a box for the raster's draw
// (`record_draw_cs`, raster.wgsl), a stale one behind every fresh one (pxl-engine
// `raster::STALE`).

/// The raster's boxes (pxl-engine `raster::BOX_WORDS`).
@group(0) @binding(14) var<storage, read_write> boxes: array<u32>;
/// The raster's args (pxl-engine `raster::ARGS_INSTANCES`, `raster::ARGS_CNT`).
@group(0) @binding(16) var<storage, read_write> args: array<atomic<u32>>;

/// Whether the node cache's rule forgets the node of `zoom` at column `c`, or a node beside it
/// across a face, whose answer what lies about its faces reads (forget.wgsl `forgets`,
/// `over_box`).
fn forgets_about(zoom: u32, c: vec2<i32>) -> bool {
  let boxes_cnt = min(node_cache[NODE_CACHE_BOXES_AT], NODE_CACHE_BOXES);
  for (var dy = -1i; dy <= 1i; dy += 1i) {
    for (var dx = -1i; dx <= 1i; dx += 1i) {
      let at = vec2<i32>(wrap_idx(zoom, c.x + dx), c.y + dy);
      if (forgets(zoom, at)) {
        return true;
      }
      for (var b = 0u; b < boxes_cnt; b += 1u) {
        let w = NODE_CACHE_BOXES_AT + 1u + 4u * b;
        let changed = vec4<i32>(
          i32(node_cache[w]),
          i32(node_cache[w + 1u]),
          i32(node_cache[w + 2u]),
          i32(node_cache[w + 3u]),
        );
        if (over_box(zoom, at, changed)) {
          return true;
        }
      }
    }
  }
  return false;
}

@compute @workgroup_size(RECORD_DRAW_LANES, 1, 1)
fn record_forget_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= RECORD_ENTRIES) {
    return;
  }
  let e = record_at(id.x);
  if (node_cache[NODE_CACHE_FORGETS] == FORGETS_ALL) {
    atomicStore(&record[e + R_HELD], 0u);
    return;
  }
  let held = record_word(e + R_HELD);
  if ((held & RECORD_HELD) == 0u) {
    return;
  }
  let k = entry_kept(e, held);
  let c = idx_of(k.corner, k.zoom, k.roots, vec3<i32>(0i));
  if (forgets_about(k.zoom, c.xy)) {
    atomicStore(&record[e + R_HELD], 0u);
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

/// Whether slot `slot`, holding `k`, is a second entry of its key: a slot before it in its
/// bucket holds `k` (two claims that each missed the other, pxl-engine `record`). It then
/// gives that one the faces it holds that one does not, with their words, and goes free.
fn joins_first(slot: u32, k: Kept) -> bool {
  let b = slot & ~(RECORD_BUCKET - 1u);
  for (var i = b; i < slot; i += 1u) {
    let f = record_at(i);
    let held = record_word(f + R_HELD);
    if ((held & RECORD_HELD) != 0u && entry_holds(f, held, k)) {
      let e = record_at(slot);
      let mine = record_word(e + R_STATE) & RECORD_FACES;
      let gained = mine & ~atomicOr(&record[f + R_STATE], mine);
      for (var axis = 0u; axis < 3u; axis += 1u) {
        if (((gained >> axis) & 1u) != 0u) {
          atomicStore(&record[f + R_FACES + axis], record_word(e + R_FACES + axis));
        }
      }
      atomicStore(&record[e + R_HELD], 0u);
      return true;
    }
  }
  return false;
}

@compute @workgroup_size(RECORD_DRAW_LANES, 1, 1)
fn record_draw_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= RECORD_ENTRIES) {
    return;
  }
  let e = record_at(id.x);
  let held = record_word(e + R_HELD);
  if ((held & RECORD_HELD) == 0u) {
    return;
  }
  let k = entry_kept(e, held);
  if (joins_first(id.x, k)) {
    return;
  }
  let o = vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
  let p = kept_planes(k, o);
  // The cell its asset keeps inset as it fills (pxl-engine cast.rs `asset_box`).
  let eighths = (record_word(e + R_ASSET) >> FILL_SHIFT) & (EIGHTHS - 1u);
  var lo = p.lo;
  var hi = p.hi;
  if (eighths != 0u) {
    let inset = (hi - lo) * (0.5 * (1.0 - side(eighths)));
    lo += inset;
    hi -= inset;
  }
  let tan_xy = vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u));
  let near = is_eyed(lo, hi, tan_xy);
  if (!near && !in_view(lo, hi)) {
    return;
  }
  let state = record_word(e + R_STATE);
  let fresh = kept_fresh(k, (state & RECORD_WHOLE) != 0u, o);
  let box_at = atomicAdd(&args[RASTER_ARGS_CNT], 1u);
  if (box_at >= RECORD_ENTRIES) {
    return;
  }
  atomicMax(&args[RASTER_ARGS_INSTANCES], box_at + 1u);
  let at = box_at * RASTER_BOX_WORDS;
  boxes[at + RASTER_BOX_LO] = bitcast<u32>(lo.x);
  boxes[at + RASTER_BOX_LO + 1u] = bitcast<u32>(lo.y);
  boxes[at + RASTER_BOX_LO + 2u] = bitcast<u32>(lo.z);
  boxes[at + RASTER_BOX_HI] = bitcast<u32>(hi.x);
  boxes[at + RASTER_BOX_HI + 1u] = bitcast<u32>(hi.y);
  boxes[at + RASTER_BOX_HI + 2u] = bitcast<u32>(hi.z);
  boxes[at + RASTER_BOX_ID] = (id.x + 1u) | select(0u, RASTER_LACK, (state & RECORD_LACKS) != 0u);
  boxes[at + RASTER_BOX_FACES] = (state & RECORD_FACES) | select(0u, RASTER_EYED, near) | select(RASTER_STALE, 0u, fresh);
}
