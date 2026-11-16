// pxl-render/frame/shader/queue.wgsl

// The holes as the queue's items (queue_walk.wgsl, queue_last.wgsl). The holes are the pixels
// the eye's cast listed and the slots the light's cast listed, which took neither a visible
// voxel nor the sky (cast.wgsl, light.wgsl). The observer's part (hole.eye.wgsl,
// hole.light.wgsl) says where its holes, pool and hits lie, a hole's ray, and the words an
// ended walk leaves.
//
// A ray parked between the run's walks lies in its observer's pool with its pixel or slot
// (`ray::PARK_WORDS`), and its item is then its place in the pool, marked by `POOLED`. A ray
// that finds the pool full ends where it stands.

/// The bit that marks an item as a parked ray, with its place in the pool in the bits under it.
const POOLED = 0x80000000u;

/// A hole's pixel or slot, from a word that holds its column in the low half and its row in
/// the high half.
fn hole_at(hole: u32) -> vec2<u32> {
  return vec2<u32>(hole & 0xffffu, hole >> 16u);
}

fn park_at(item: u32) -> u32 {
  return pool_at() + (item & ~POOLED) * PARK_WORDS;
}

/// A hole's pixel or slot. A parked ray's is read from its place in the pool.
fn hole_px(item: u32) -> vec2<u32> {
  if ((item & POOLED) != 0u) {
    return hole_at(beams[park_at(item) + PARK_HOLE]);
  }
  return hole_at(item);
}

/// The next of the observer's holes for the first walk to take, or `NONE` where it takes no
/// more. The count is held to the list's room.
fn next_item() -> u32 {
  let at = holes_at();
  let k = take_below(QUEUE_NEXT, in_cap(beams[at], holes_cap()));
  if (k == NONE) {
    return NONE;
  }
  return beams[at + 1u + k];
}

/// The next ray the first walk parked for the second walk to take, or `NONE` where it takes no
/// more. The count is held to the pool's room.
fn next_parked() -> u32 {
  return parked_item(take_below(QUEUE_NEXT_PARKED, in_cap(load(QUEUE_PARKED), POOL_CAP)));
}

/// Place `k` in the pool as an item, or `NONE`.
fn parked_item(k: u32) -> u32 {
  return select(POOLED | k, NONE, k == NONE);
}

/// The next parked ray for the last walk to take from the pool, or `NONE` past the rays
/// parked.
fn next_last() -> u32 {
  return parked_item(take_below(LAST_NEXT, in_cap(load(LAST_CNT), POOL_CAP)));
}

/// Parks a hole's ray at its node `corner` of `zoom` with `roots`, `t` along the ray, after
/// `step` steps (parked.wgsl). A ray already parked keeps its place in the pool. A hole not yet
/// parked takes the next place and stores its pixel or slot there. It returns false where the
/// pool is full.
fn park(item: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) -> bool {
  var pooled = item;
  if ((item & POOLED) == 0u) {
    let k = take(QUEUE_PARKED);
    if (k >= POOL_CAP) {
      return false;
    }
    pooled = POOLED | k;
    beams[park_at(pooled) + PARK_HOLE] = item;
  }
  let at = park_at(pooled);
  beams[at] = bitcast<u32>(t);
  beams[at + 1u] = corner.x;
  beams[at + 2u] = corner.y;
  beams[at + 3u] = corner.z;
  beams[at + 4u] = parked_word(zoom, roots, step);
  return true;
}

fn parked(item: u32) -> bool {
  return (beams[park_at(item) + 4u] & PARKED) != 0u;
}

fn parked_start(item: u32) -> Start {
  let at = park_at(item);
  return parked_from(beams[at], vec3<u32>(beams[at + 1u], beams[at + 2u], beams[at + 3u]), beams[at + 4u]);
}

/// Takes a number for a hit the record will take, as the walk that ends the ray numbers them
/// (`hit_number`). The number that opens a workgroup raises the dispatch over the hits. Past
/// the list's room it is `NONE` (`ray::HITS_A_FRAME`).
fn hit_in_cap() -> u32 {
  let r = hit_number();
  if (r >= HITS_A_FRAME) {
    return NONE;
  }
  if (r % HIT_LANE_CNT == 0u) {
    hit_rows(workgroup_cnt(r + 1u, HITS_A_FRAME, HIT_LANE_CNT));
  }
  return r;
}

/// Lists hit `r` for the record (miss_word.wgsl `hit_put`) and returns its number marked as
/// waiting (`beam::HIT_WAIT`). The pixel keeps that word until record_hits.wgsl puts there
/// what lies around the hit's face.
fn keep_hit(r: u32, w: WalkEnd, face: u32, id: vec2<u32>) -> u32 {
  hit_put(r, w, face, id);
  return HIT_WAIT | r;
}

/// Ends the walk `w` of hole `item`. Its hit, whatever it stopped on, is listed for its
/// observer's record (`keep_hit`, record_hits.wgsl), the eye's and the light's alike, and the
/// observer writes what the ray shows at the hole (`hole_write`): for a whole cell of the eye,
/// with the word the record puts there once its hit is in (record_hits.wgsl `rec_faces_cs`).
/// A ray that ended as a lack shows the deepest node held (`shows`), and its hit stays a
/// lack, so the next frame asks for what it lacked.
fn finish(item: u32, aim: Aim, w: WalkEnd) {
  let hit = w.ends == ENDS_CELL || w.ends == ENDS_LACK;
  var r = NONE;
  if (hit) {
    r = hit_in_cap();
  }
  let at = hole_px(item);
  if ((item & POOLED) != 0u) {
    beams[park_at(item) + 4u] = 0u;
  }
  let face = face_of(aim, w);
  let shown = shows(aim, w);
  var faces = 0u;
  // A `select` evaluates both its arms, so the hit is kept under an `if`, only where it has a
  // number.
  if (r != NONE) {
    faces = keep_hit(r, w, face | select(0u, HIT_LACK, w.ends == ENDS_LACK), at);
  }
  hole_write(at, w, shown, face, faces);
}
