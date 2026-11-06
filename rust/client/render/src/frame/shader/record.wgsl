// pxl-render/record.wgsl

// The eye's record of voxels, kept across frames (pxl-engine record.rs): its slots, a node's
// key and bucket, a slot's node as a walk from the eye holds it, and whether that node is the
// one the eye's walk stops on. Its writers and their order are record.rs's.

@group(0) @binding(23) var<storage, read_write> record: array<atomic<u32>>;

/// Word `i` of the record.
fn record_word(i: u32) -> u32 {
  return atomicLoad(&record[i]);
}

/// The frame's number (`record::FRAME`).
fn record_frame() -> u32 {
  return record_word(RECORD_FRAME);
}

/// The first word of slot `slot`'s entry (`record::entry_at`).
fn record_at(slot: u32) -> u32 {
  return RECORD_HEAD_WORDS + slot * RECORD_ENTRY_WORDS;
}

/// A node as the record keys it, its roots world-absolute (`record::Key`).
struct Kept {
  zoom: u32,
  corner: vec3<u32>,
  roots: vec3<i32>,
  band: u32,
}

/// Its held word (`Key::held`).
fn kept_held(k: Kept) -> u32 {
  return RECORD_HELD | (k.band << 8u) | k.zoom;
}

/// The first slot of its bucket (`record::bucket_of`).
fn kept_bucket(k: Kept) -> u32 {
  var h = kept_held(k) * 0x85EBCA77u;
  h = mix_hash(h, k.corner.x);
  h = mix_hash(h, k.corner.y);
  h = mix_hash(h, k.corner.z);
  h = mix_hash(h, u32(k.roots.x));
  h = mix_hash(h, u32(k.roots.y));
  h = mix_hash(h, u32(k.roots.z));
  return (h ^ (h >> 16u)) & (RECORD_ENTRIES - 1u) & ~(RECORD_BUCKET - 1u);
}

/// The claim word of a slot being written for it (`record::claim_of`).
fn kept_claim(k: Kept) -> u32 {
  var h = kept_held(k) * 0x27D4EB2Fu;
  h = mix_hash(h, k.corner.x);
  h = mix_hash(h, k.corner.y);
  h = mix_hash(h, k.corner.z);
  h = mix_hash(h, u32(k.roots.x));
  h = mix_hash(h, u32(k.roots.y));
  h = mix_hash(h, u32(k.roots.z));
  return RECORD_CLAIM | ((h ^ (h >> 16u)) & (RECORD_CLAIM - 1u));
}

/// The root holding the origin `o` on each axis (`cast::roots_of`).
fn origin_roots(o: vec3<i32>) -> vec3<i32> {
  return vec3<i32>(o.x >> 31u, 0i, i32(u32(o.z) + Z_START) >> 31u);
}

/// The key of the entry at word `e`, its held word `held`.
fn entry_kept(e: u32, held: u32) -> Kept {
  return Kept(
    held & 0xffu,
    vec3<u32>(record_word(e + R_CORNER), record_word(e + R_CORNER + 1u), record_word(e + R_CORNER + 2u)),
    vec3<i32>(
      i32(record_word(e + R_ROOTS)),
      i32(record_word(e + R_ROOTS + 1u)),
      i32(record_word(e + R_ROOTS + 2u)),
    ),
    (held & ~RECORD_HELD) >> 8u,
  );
}

/// Whether the entry at word `e`, its held word `held`, holds `k`.
fn entry_holds(e: u32, held: u32, k: Kept) -> bool {
  let c = vec3<u32>(record_word(e + R_CORNER), record_word(e + R_CORNER + 1u), record_word(e + R_CORNER + 2u));
  let r = vec3<u32>(record_word(e + R_ROOTS), record_word(e + R_ROOTS + 1u), record_word(e + R_ROOTS + 2u));
  return held == kept_held(k) && all(c == k.corner) && all(r == vec3<u32>(k.roots));
}

/// The node's planes in units from the origin `o` (pxl-engine cast.rs `bounds`).
struct Planes {
  lo: vec3<f32>,
  hi: vec3<f32>,
}

fn kept_planes(k: Kept, o: vec3<i32>) -> Planes {
  let og = vec3<u32>(u32(o.x), u32(o.y) + Y_START, u32(o.z) + Z_START);
  let roots = k.roots - origin_roots(o);
  return Planes(units(k.corner, og, roots), far_of(k.corner, under(k.zoom), og, roots));
}

/// The zoom the eye's walk asks a node of planes `p` at its nearest point, as walk.wgsl
/// asks it (`cast::node_zooms`).
fn asked_near(p: Planes) -> u32 {
  let range = vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u));
  if (range.x == range.y) {
    return range.x;
  }
  return zoom_at(squared(min(max(p.lo, vec3<f32>(0.0)), p.hi)), range);
}

/// Whether the eye's ray from `o` stopped on `k` whole, coarser than it asked there
/// (`record::stopped_whole`).
fn stopped_whole(k: Kept, o: vec3<i32>) -> bool {
  return k.zoom < asked_near(kept_planes(k, o));
}

/// Whether `k`, `whole` where it stopped whole, is the node the eye's walk from `o` stops on:
/// it answers a cell and its parent does not (`record::fresh`).
fn kept_fresh(k: Kept, whole: bool, o: vec3<i32>) -> bool {
  if (!whole && k.zoom < asked_near(kept_planes(k, o))) {
    return false;
  }
  if (k.zoom == 0u) {
    return true;
  }
  let zoom = k.zoom - 1u;
  let parent = Kept(zoom, k.corner & vec3<u32>(~under(zoom)), k.roots, k.band);
  return zoom < asked_near(kept_planes(parent, o));
}
