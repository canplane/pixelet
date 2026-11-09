// pxl-render/queue.wgsl

// The holes as the queue's items (queue_walk.wgsl, queue_last.wgsl), for either observer. The
// holes are the pixels the eye's cast listed and the slots the light's cast listed, which took
// neither a visible voxel nor the sky (cast.wgsl, light.wgsl). The observer is read off the
// head the pass binds (light_aim.wgsl `light_run`). One code serves both, and the observer
// chooses the pixel or slot, the ray and the words written.
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

fn eye_size() -> vec2<u32> {
  return textureDimensions(rays).xy;
}

fn pool_at() -> u32 {
  if (light_run()) {
    return header_word(LIGHT_HEADER_POOL);
  }
  return eye_pool_at(eye_size());
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

/// Where the observer's stops lie (hit.wgsl): the eye's past the beam rays' words
/// (`beam::Grid::hits_at`), and the light's past its pool (`light::hits_at`).
fn hits_at() -> u32 {
  if (light_run()) {
    return header_word(LIGHT_HEADER_HITS);
  }
  return eye_word(GRID + GRID_HITS_AT);
}

/// Where the observer's listed holes lie: their count, then their entries (`ray::holes_at`,
/// `light::holes_at`).
fn holes_at() -> u32 {
  if (light_run()) {
    return header_word(LIGHT_HEADER_HOLES);
  }
  return eye_holes_at(eye_size());
}

/// The next item for a walk over the node cache to take, or `NONE` where it takes no more. The
/// first walk takes the observer's holes, and the `parked` takes the rays the first parked.
fn next_item(parked: bool) -> u32 {
  if (parked) {
    return parked_item(take_below(QUEUE_NEXT_PARKED, min(load(QUEUE_PARKED), POOL_CAP)));
  }
  let at = holes_at();
  let k = take_below(QUEUE_NEXT, beams[at]);
  if (k == NONE) {
    return NONE;
  }
  return beams[at + 1u + k];
}

/// Place `k` in the pool as an item, or `NONE`.
fn parked_item(k: u32) -> u32 {
  return select(POOLED | k, NONE, k == NONE);
}

/// The next parked ray for the last walk to take from the pool, or `NONE` past the rays
/// parked.
fn next_last() -> u32 {
  return parked_item(take_below(LAST_NEXT, min(load(LAST_CNT), POOL_CAP)));
}

fn item_aim(item: u32) -> Aim {
  let at = hole_px(item);
  if (light_run()) {
    return light_aim(slot_texel(at));
  }
  return px_aim(at, eye_size());
}

/// Where a hole's ray starts afresh: at the start of its cell.
fn item_start(item: u32, aim: Aim) -> Start {
  let at = hole_px(item);
  if (light_run()) {
    return light_resumed(aim, slot_texel(at));
  }
  return px_start(aim, at, eye_size());
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

/// Takes a number for a stop the record will take, as the walk that ends the ray numbers them
/// (`hit_number`). The number that opens a workgroup raises the dispatch over the stops. Past
/// the list's room it is `NONE` (`ray::HITS_A_FRAME`).
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

/// Lists stop `r` for the record (miss_word.wgsl `hit_put`) and returns its number marked as
/// waiting (`beam::HIT_WAITS`). The pixel keeps that word until record_hits.wgsl puts there
/// what lies around the stop's face.
fn keep_hit(r: u32, w: WalkEnd, face: u32, id: vec2<u32>) -> u32 {
  hit_put(r, w, face, id);
  return HIT_WAITS | r;
}

/// Ends the walk `w` of hole `item`. Its stop, whatever it stopped on, is listed for its
/// observer's record (`keep_hit`, record_hits.wgsl), the eye's and the light's alike. A slot of
/// the light gets its depth. A pixel of the eye gets its ray's words, and what lies around the
/// face it stopped on: a box's for a cell short of its cube (pxl-engine `face::BOX`), and for a
/// whole cell the word the record puts there once its stop is in (record_hits.wgsl
/// `record_faces_cs`). A ray that ended as a lack shows the deepest node held (`shows`), and
/// its stop stays a lack, so the next frame asks for what it lacked.
fn finish(item: u32, aim: Aim, w: WalkEnd) {
  let stops = w.ends == 1u || w.ends == 2u;
  var r = NONE;
  if (stops) {
    r = hit_in_cap();
  }
  let at = hole_px(item);
  if ((item & POOLED) != 0u) {
    beams[park_at(item) + 4u] = 0u;
  }
  let face = face_of(aim, w);
  let shown = shows(aim, w);
  if (light_run()) {
    if (r != NONE) {
      _ = keep_hit(r, w, face | select(0u, HIT_LACKS, w.ends == 2u), at);
    }
    textureStore(rays, at, 0u, vec4<u32>(walked_word(shown), 0u, 0u, 0u));
    return;
  }
  var faces = 0u;
  // A `select` evaluates both its arms, so the stop is kept under an `if`, only where it has a
  // number.
  if (r != NONE) {
    faces = keep_hit(r, w, face | select(0u, HIT_LACKS, w.ends == 2u), at);
  }
  // A lack's stop takes no word from the record, so what its pixel shows waits for none.
  if (w.ends == 2u) {
    faces = 0u;
  }
  if (shown.ends == 1u && hit_fill(shown) != 0u) {
    faces = FACE_BOX;
  }
  var word = ray_word(shown, face, faces);
  // In the ray steps view, the steps of its beam ray and of its own walk.
  if (eye_word(STEPS) != 0u) {
    word.z = beam_steps(grid_cell(px_tan(at, eye_size()))) + w.step;
  }
  write_ray(at, word);
}
