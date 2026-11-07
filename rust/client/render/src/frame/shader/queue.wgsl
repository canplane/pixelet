// pxl-render/queue.wgsl

// The holes as the queue's items (queue_walk.wgsl, queue_last.wgsl; pxl-engine
// `ray::QUEUE_LANES`), an observer's: the pixels the eye's cast marked and the slots the light's
// listed, which no voxel answers (cast.wgsl, light.wgsl). The observer is the head the pass binds
// (light_aim.wgsl `light_run`): one code for both, its pixel or slot, its ray, its start and its
// words chosen by it.
//
// The light's holes are its list's slots, a ray parked at its slot's place in its holes' region
// (`light_state_at`). A slot still parked at its queue's end is owed (pxl-engine
// `node_cache::OWED_AT`): its tile to the next claim, which takes its state where it resumes
// (`light::READ_OWED`; light_propagate.wgsl).
//
// The eye's holes are its owed bits (walked.wgsl): the first walk takes a word of them at a time
// and clears it, each lane walking its word's holes one after another, each from its cell's
// start; first, in a run alone, the rays the run before parked. A ray is parked as its walk's
// state, with its pixel, in the run's pool (`ray::pool_at`), and its item is then its place
// there (`SLOT`), which the later walks take. The first walk takes another hole only while every
// lane could park within the pool's room (`ray::PARKED_ROOM`) and the frame's budget is not
// spent; the holes it does not take keep their owed bits. A pixel still owed at the run's end,
// an owed bit or a ray parked in its pool, is owed to the next frame's queue while the rays
// stand (parked_tiles.wgsl).

/// An eye's item that is a parked ray, not a pixel: the pool it lies in at `SLOT_POOL`, its
/// place there under it.
const SLOT = 0x80000000u;
const SLOT_POOL = 30u;
const SLOT_PLACE = 0x3fffffffu;

/// The owed bits' word a lane of the first walk holds, and its holes the lane has not taken.
var<private> held_word: u32;
var<private> held_bits: u32;

/// A light's hole's slot, its column under its row.
fn hole_at(hole: u32) -> vec2<u32> {
  return vec2<u32>(hole & 0xffffu, hole >> 16u);
}

/// The eye's drawn pixels each way.
fn eye_size() -> vec2<u32> {
  return textureDimensions(rays).xy;
}

/// What the eye's run takes (`ray::from_at`): its cast's owed bits, or a run alone's and the
/// pool the run before parked in.
fn eye_from() -> u32 {
  return beams[from_at(eye_size())];
}

/// The pool the eye's run parks its rays in.
fn eye_into() -> u32 {
  return owed_into(eye_from());
}

/// The rays parked in pool `k` as its run counted them, no more than its room.
fn pool_cnt(k: u32) -> u32 {
  return min(beams[pool_at(eye_size(), k) + POOL_CNT], PARKED_ROOM);
}

/// The eye's item that is the ray parked at place `k` of pool `pool`.
fn slot_item(pool: u32, k: u32) -> u32 {
  return SLOT | (pool << SLOT_POOL) | k;
}

/// The first word of an eye's parked ray's state in its pool (`ray::PARK_WORDS`).
fn park_at(item: u32) -> u32 {
  return pool_at(eye_size(), (item >> SLOT_POOL) & 1u) + POOL_STATES + (item & SLOT_PLACE) * PARK_WORDS;
}

/// A hole's pixel or slot: a light's slot, an eye's pixel, or the pixel its parked ray keeps.
fn hole_px(hole: u32) -> vec2<u32> {
  if (!light_run() && (hole & SLOT) != 0u) {
    return hole_at(beams[park_at(hole) + PARK_PX]);
  }
  return hole_at(hole);
}

/// The slots a light's cast can list: those of the tiles it claims (pxl-engine
/// `light::parked_room`).
fn light_room() -> u32 {
  return LIGHT_PARKED_ROOM;
}

/// Where the observer's stops lie and their room (hit.wgsl): the eye's past the beam rays'
/// words (`beam::Grid::hits_at`); the light's past its holes' region (`light::hits_at`); a
/// frame's stops each (`ray::STOPS_A_FRAME`).
fn hits_at() -> u32 {
  if (light_run()) {
    return head_word(LIGHT_HEAD_HITS);
  }
  return eye_word(GRID + GRID_HITS_AT);
}

fn hits_room() -> u32 {
  return STOPS_A_FRAME;
}

/// The light's slot at place `k` of its cast's list (`light::misses_at`), `NONE` past its count.
fn light_item(k: u32) -> u32 {
  if (k >= in_room(beams[head_word(LIGHT_HEAD_MISSES_CNT)], light_room())) {
    return NONE;
  }
  return beams[head_word(LIGHT_HEAD_MISSES) + k];
}

/// The next item a walk over the node cache takes, the first or the one `again`; `NONE` where
/// it takes no more. The light's, its list's next slot. The eye's second walk takes the rays the
/// first parked in the run's pool; its first, in a run alone, the rays the run before parked,
/// then the holes of its owed bits, a word at a time, while every lane could park within the
/// pool's room and the frame's budget is not `spent`, a lane that stops setting back the bits it
/// did not take.
fn next_item(again: bool, spent: bool) -> u32 {
  if (light_run()) {
    return light_item(take(select(QUEUE_TAKEN, QUEUE_TAKEN_AGAIN, again)));
  }
  if (again) {
    let k = take(QUEUE_TAKEN_AGAIN);
    if (k >= min(held(QUEUE_PARKED), PARKED_ROOM)) {
      return NONE;
    }
    return slot_item(eye_into(), k);
  }
  let taking = eye_from();
  if (taking >= FROM_OWED) {
    let carried = taking - FROM_OWED;
    let k = take(QUEUE_TAKEN);
    if (k < pool_cnt(carried)) {
      return slot_item(carried, k);
    }
  }
  let size = eye_size();
  let owed = owed_bits_at(size);
  let more = !spent && held(QUEUE_PARKED) + QUEUE_GROUPS * QUEUE_LANES < PARKED_ROOM;
  if (!more) {
    if (held_bits != 0u) {
      beams[owed + held_word] = beams[owed + held_word] | held_bits;
      held_bits = 0u;
    }
    return NONE;
  }
  while (held_bits == 0u) {
    let w = take(QUEUE_SQUARES);
    if (w >= hole_words(size)) {
      return NONE;
    }
    held_word = w;
    held_bits = beams[owed + w];
    beams[owed + w] = 0u;
  }
  let bit = firstTrailingBit(held_bits);
  held_bits = held_bits & (held_bits - 1u);
  let p = bit_px(held_word, bit, size);
  return p.x | (p.y << 16u);
}

/// The next item the last walk takes, `NONE` where it takes no more: the light's list's next
/// slot; the eye's next ray of the run's pool.
fn next_last() -> u32 {
  let k = take(LAST_TAKEN);
  if (light_run()) {
    return light_item(k);
  }
  let into = eye_into();
  if (k >= pool_cnt(into)) {
    return NONE;
  }
  return slot_item(into, k);
}

/// The first word of a hole's state: a light's slot's at its tile's place and its slot within
/// the tile, in its cast's region (light_aim.wgsl `light_state_at`); an eye's parked ray's in
/// its pool.
fn state_at(hole: u32) -> u32 {
  if (light_run()) {
    let at = hole_at(hole);
    let place = beams[light_place_at(at)];
    let k = (at.y % LIGHT_TILE) * LIGHT_TILE + at.x % LIGHT_TILE;
    return light_state_at(place, k, head_word(LIGHT_HEAD_CAST));
  }
  return park_at(hole);
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

/// A hole's ray parked at its node `corner`, `zoom`, `roots`, `t` along it, `step` steps taken,
/// at its state (parked.wgsl): a light's at its slot's; an eye's where it lies in the run's pool,
/// or at the next place there, its pixel with it, where it lies in none, a hole taken off its
/// owed bit or a ray of the run before's pool. Only the walks over the node cache park an eye's
/// ray outside the run's pool, which their counter of it numbers (`ray::QUEUE_PARKED`).
fn park(hole: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) {
  var at = 0u;
  if (light_run()) {
    at = state_at(hole);
  } else {
    let into = eye_into();
    if ((hole & SLOT) != 0u && ((hole >> SLOT_POOL) & 1u) == into) {
      at = park_at(hole);
    } else {
      let p = hole_px(hole);
      at = park_at(slot_item(into, take(QUEUE_PARKED)));
      if (at + PARK_PX < arrayLength(&beams)) {
        beams[at + PARK_PX] = p.x | (p.y << 16u);
      }
    }
  }
  if (at + 4u >= arrayLength(&beams)) {
    return;
  }
  beams[at] = bitcast<u32>(t);
  beams[at + 1u] = corner.x;
  beams[at + 2u] = corner.y;
  beams[at + 3u] = corner.z;
  beams[at + 4u] = parked_word(zoom, roots, step);
}

/// Whether a hole's ray is parked: an eye's pixel taken off its owed bit never is.
fn parked(hole: u32) -> bool {
  if (!light_run() && (hole & SLOT) == 0u) {
    return false;
  }
  return (beams[state_at(hole) + 4u] & PARKED) != 0u;
}

/// What a hole's first walk of a run takes: the state it is parked in, a light's slot of an
/// owed tile, whose claim took its state (light_propagate.wgsl `light_claim_cs`), or an eye's
/// ray the run before parked; a fresh start where none is parked, but an eye's ray of a pool
/// that has ended. The second walk takes those the first parked.
fn taken(hole: u32, again: bool) -> u32 {
  if (parked(hole)) {
    return TAKEN_PARKED;
  }
  if (!light_run() && (hole & SLOT) != 0u) {
    return TAKEN_NONE;
  }
  return select(TAKEN_FRESH, TAKEN_NONE, again);
}

/// The state a hole's ray is parked in, where a walk goes on from.
fn parked_start(hole: u32) -> Start {
  let at = state_at(hole);
  return parked_from(beams[at], vec3<u32>(beams[at + 1u], beams[at + 2u], beams[at + 3u]), beams[at + 4u]);
}

/// A hole's ray no longer parked, where it is.
fn unpark(hole: u32) {
  if (light_run() || (hole & SLOT) != 0u) {
    beams[state_at(hole) + 4u] = 0u;
  }
}

/// A stop's number for the record, as the walk that ends it numbers them (`stop_number`),
/// the dispatch over them raised by the number that opens a workgroup; `NONE` past its
/// observer's room (`hits_room`, pxl-engine `ray::STOPS_A_FRAME`).
fn stop_room() -> u32 {
  let r = stop_number();
  if (r >= hits_room()) {
    return NONE;
  }
  if (r % HIT_LANES == 0u) {
    stop_rows(workgroups(r + 1u, hits_room(), HIT_LANES));
  }
  return r;
}

/// Stop `r` listed for the record, what lies about its face put at its pixel off it
/// (miss_word.wgsl `hit_put`, record_stops.wgsl), and its number marked waiting
/// (`beam::HIT_WAITS`).
fn keep_hit(r: u32, w: Walked, face: u32, id: vec2<u32>) -> u32 {
  hit_put(r, w, face, id);
  return HIT_WAITS | r;
}

/// Hole `hole`'s walk `w`, ended, its words written and its stop, whatever it stopped on,
/// listed for its observer's record (`keep_hit`, record_stops.wgsl), the eye's and the
/// light's alike: a slot's depth; the eye's ray's, what lies about the face it stopped on its
/// box's for a cell short of its cube (pxl-engine `face::BOX`), a lack's none, a whole cell's
/// put there off the record once its stop is in (record_stops.wgsl `record_faces_cs`). A
/// parked ray keeps no stop until it ends. Whether it ended: a stop past the frame's room
/// (`stop_room`) leaves the hole as it was, for its walk to park and owe.
fn finish(hole: u32, aim: Aim, w: Walked) -> bool {
  let stops = w.ends == 1u || w.ends == 2u;
  var r = NONE;
  if (stops) {
    r = stop_room();
    if (r == NONE) {
      return false;
    }
  }
  unpark(hole);
  let at = hole_px(hole);
  let face = face_of(aim, w);
  if (light_run()) {
    if (stops) {
      _ = keep_hit(r, w, face | select(0u, HIT_LACKS, w.ends == 2u), at);
    }
    textureStore(rays, at, 0u, vec4<u32>(walked_word(w), 0u, 0u, 0u));
    return true;
  }
  var faces = 0u;
  // Both of a `select` are taken, so the stop is kept under an `if`.
  if (stops) {
    faces = keep_hit(r, w, face | select(0u, HIT_LACKS, w.ends == 2u), at);
    if (w.ends == 1u && hit_fill(w) != 0u) {
      faces = FACE_BOX;
    }
  }
  write_ray(at, ray_word(w, face, faces));
  return true;
}

/// A hole's ray the frame's budget does not reach, at the node `corner`, `zoom`, `roots`, `t`
/// along it, `step` steps taken: parked where its state lies, its word left as the cast wrote
/// it. An eye's pixel keeps what the record drew there (cast.wgsl), its ray in the run's pool,
/// which the frame's queue alone takes on while the rays stand; it is counted as owed with the
/// owed bits (parked_tiles.wgsl). A light's slot's tile is owed (pxl-engine `light::READ_OWED`),
/// so the next claim takes it nearest first and the ray goes on from there; its word written
/// when it ends, which that cast's lay of the tiles read takes (gpu/shadow.rs `begin`), and
/// counted as its observer's owed work (`node_cache::OWED_AT`), which the renderer reads back
/// and wakes the next frame by. The tile's read word is the claim's and the slide's to write but
/// for this bit, which every slot of the tile it sets sets alike.
fn unreached(hole: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) {
  park(hole, t, corner, zoom, roots, step);
  if (!light_run()) {
    return;
  }
  let tile = slot_texel(hole_at(hole)) / LIGHT_TILE;
  let w = head_word(LIGHT_HEAD_READS) + tile.y * head_word(LIGHT_HEAD_TILES) + tile.x;
  if (w < arrayLength(&beams)) {
    beams[w] = beams[w] | LIGHT_READ_OWED;
  }
  atomicAdd(&node_cache[LIGHT_OWED], 1u);
}
