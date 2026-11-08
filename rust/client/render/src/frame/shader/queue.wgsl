// pxl-render/queue.wgsl

// The holes as the queue's items (queue_walk.wgsl, queue_last.wgsl), an observer's: the pixels
// the eye's cast marked and the slots the light's listed, which no voxel answers (cast.wgsl,
// light.wgsl). The observer is the head the pass binds (light_aim.wgsl `light_run`): one code
// for both, its pixel or slot, its ray and its words chosen by it.
//
// A ray parked between the run's walks lies in its observer's pool with its pixel or slot
// (`ray::PARK_WORDS`), and its item is then its place there (`SLOT`). Past the pool's room it
// ends where it stands.

/// An item that is a parked ray, its place in the pool under it.
const SLOT = 0x80000000u;

/// A hole's slot, its column under its row.
fn hole_at(hole: u32) -> vec2<u32> {
  return vec2<u32>(hole & 0xffffu, hole >> 16u);
}

/// The eye's drawn pixels each way.
fn eye_size() -> vec2<u32> {
  return textureDimensions(rays).xy;
}

/// The observer's pool's first word (`ray::pool_at`, `light::HEADER_POOL`).
fn pool_at() -> u32 {
  if (light_run()) {
    return header_word(LIGHT_HEADER_POOL);
  }
  return eye_pool_at(eye_size());
}

/// The first word of parked ray `item` in the pool (`ray::PARK_WORDS`).
fn park_at(item: u32) -> u32 {
  return pool_at() + POOL_STATES + (item & ~SLOT) * PARK_WORDS;
}

/// A hole's pixel or slot, a parked ray's off its pool.
fn hole_px(hole: u32) -> vec2<u32> {
  if ((hole & SLOT) != 0u) {
    return hole_at(beams[park_at(hole) + PARK_PX]);
  }
  return hole_at(hole);
}

/// Where the observer's stops lie (hit.wgsl): the eye's past the beam rays' words
/// (`beam::Grid::hits_at`); the light's past its pool (`light::hits_at`).
fn hits_at() -> u32 {
  if (light_run()) {
    return header_word(LIGHT_HEADER_HITS);
  }
  return eye_word(GRID + GRID_HITS_AT);
}

/// The observer's listed holes' count, their entries after it (`ray::holes_at`,
/// `light::holes_at`).
fn holes_at() -> u32 {
  if (light_run()) {
    return header_word(LIGHT_HEADER_HOLES);
  }
  return eye_holes_at(eye_size());
}

/// The next item a walk over the node cache takes; `NONE` where it takes no more. The second
/// takes the rays the first parked, the first the observer's holes.
fn next_item(again: bool) -> u32 {
  if (again) {
    let k = take(QUEUE_NEXT_AGAIN);
    return select(NONE, SLOT | k, k < min(held(QUEUE_PARKED), POOL_CAP));
  }
  let k = take(QUEUE_NEXT);
  let at = holes_at();
  return select(NONE, beams[at + 1u + k], k < beams[at]);
}

/// The next ray the last walk takes off the pool, `NONE` past it.
fn next_last() -> u32 {
  let k = take(LAST_NEXT);
  return select(NONE, SLOT | k, k < min(beams[pool_at() + POOL_CNT], POOL_CAP));
}

/// A hole's ray.
fn item_aim(hole: u32) -> Aim {
  let at = hole_px(hole);
  if (light_run()) {
    return light_aim(slot_texel(at));
  }
  return px_aim(at, eye_size());
}

/// Where a hole's ray starts afresh: its cell's start.
fn item_start(hole: u32, aim: Aim) -> Start {
  let at = hole_px(hole);
  if (light_run()) {
    return light_resumed(aim, slot_texel(at));
  }
  return px_start(aim, at, eye_size());
}

/// A hole's ray parked at its node `corner`, `zoom`, `roots`, `t` along it, `step` steps taken
/// (parked.wgsl): where it lies in the pool, or at the next place there, its pixel with it;
/// false past the pool's room.
fn park(hole: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) -> bool {
  var item = hole;
  if ((hole & SLOT) == 0u) {
    let k = take(QUEUE_PARKED);
    if (k >= POOL_CAP) {
      return false;
    }
    item = SLOT | k;
    beams[park_at(item) + PARK_PX] = hole;
  }
  let at = park_at(item);
  beams[at] = bitcast<u32>(t);
  beams[at + 1u] = corner.x;
  beams[at + 2u] = corner.y;
  beams[at + 3u] = corner.z;
  beams[at + 4u] = parked_word(zoom, roots, step);
  return true;
}

fn parked(hole: u32) -> bool {
  return (beams[park_at(hole) + 4u] & PARKED) != 0u;
}

/// The state a hole's ray is parked in, where a walk goes on from.
fn parked_start(hole: u32) -> Start {
  let at = park_at(hole);
  return parked_from(beams[at], vec3<u32>(beams[at + 1u], beams[at + 2u], beams[at + 3u]), beams[at + 4u]);
}

/// A stop's number for the record, as the walk that ends it numbers them (`hit_number`),
/// the dispatch over them raised by the number that opens a workgroup; `NONE` past its
/// list's room (`ray::HITS_A_FRAME`).
fn hit_in_cap() -> u32 {
  let r = hit_number();
  if (r >= HITS_A_FRAME) {
    return NONE;
  }
  if (r % HIT_LANES == 0u) {
    hit_rows(workgroups(r + 1u, HITS_A_FRAME, HIT_LANES));
  }
  return r;
}

/// Stop `r` listed for the record, what lies about its face put at its pixel off it
/// (miss_word.wgsl `hit_put`, record_hits.wgsl), and its number marked waiting
/// (`beam::HIT_WAITS`).
fn keep_hit(r: u32, w: WalkEnd, face: u32, id: vec2<u32>) -> u32 {
  hit_put(r, w, face, id);
  return HIT_WAITS | r;
}

/// Hole `hole`'s walk `w`, ended, its words written and its stop, whatever it stopped on,
/// listed for its observer's record (`keep_hit`, record_hits.wgsl), the eye's and the
/// light's alike: a slot's depth; the eye's ray's, what lies about the face it stopped on its
/// box's for a cell short of its cube (pxl-engine `face::BOX`), a lack's none, a whole cell's
/// put there off the record once its stop is in (record_hits.wgsl `record_faces_cs`).
fn finish(hole: u32, aim: Aim, w: WalkEnd) {
  let stops = w.ends == 1u || w.ends == 2u;
  var r = NONE;
  if (stops) {
    r = hit_in_cap();
  }
  let at = hole_px(hole);
  if ((hole & SLOT) != 0u) {
    beams[park_at(hole) + 4u] = 0u;
  }
  let face = face_of(aim, w);
  if (light_run()) {
    if (r != NONE) {
      _ = keep_hit(r, w, face | select(0u, HIT_LACKS, w.ends == 2u), at);
    }
    textureStore(rays, at, 0u, vec4<u32>(walked_word(w), 0u, 0u, 0u));
    return;
  }
  var faces = 0u;
  // Both of a `select` are taken, so the stop is kept under an `if`.
  if (r != NONE) {
    faces = keep_hit(r, w, face | select(0u, HIT_LACKS, w.ends == 2u), at);
  }
  if (w.ends == 1u && hit_fill(w) != 0u) {
    faces = FACE_BOX;
  }
  write_ray(at, ray_word(w, face, faces));
}
