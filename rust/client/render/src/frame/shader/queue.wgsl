// pxl-render/queue.wgsl

// The holes as the queue's items (queue_walk.wgsl, queue_last.wgsl; pxl-engine
// `ray::QUEUE_LANES`), an observer's: the pixels the eye's cast marked and the slots the light's
// listed, which no voxel answers (cast.wgsl, light.wgsl). The observer is the head the pass binds (light_aim.wgsl
// `light_run`): one code for both, its pixel or slot, its ray, its start and its words chosen
// by it. A ray is parked as its walk's state (`Walked`, walk.wgsl) at its hole's place in the
// holes' region (`state_at`), from which a later walk goes on as one unbroken walk; the region
// holds a state for every hole a cast can list, so every ray can be parked. A hole still
// parked at its queue's end stays parked and is owed (pxl-engine `node_cache::OWED_AT`): a
// light's slot's tile to the next claim, which takes its state where it resumes
// (`light::READ_OWED`; light_propagate.wgsl); an eye's pixel to the next frame's queue while
// the rays stand, its cast having set every other pixel's state to no parked ray (cast.wgsl).

/// A hole's pixel or slot, its column under its row.
fn hole_at(hole: u32) -> vec2<u32> {
  return vec2<u32>(hole & 0xffffu, hole >> 16u);
}

/// The holes a cast can list (pxl-engine `ray::region_words`): the eye's drawn pixels, the
/// slots of the tiles the light's cast claims (`light::parked_room`).
fn room() -> u32 {
  if (light_run()) {
    return LIGHT_PARKED_ROOM;
  }
  let size = textureDimensions(rays);
  return size.x * size.y;
}

/// Where the observer's stops lie and their room (hit.wgsl): the eye's past the beam rays'
/// words (`beam::Grid::hits_at`, `beam::HITS`); the light's past its holes' region, a stop each
/// of the slots its cast claims (`light::hits_at`, `light::parked_room`).
fn hits_at() -> u32 {
  if (light_run()) {
    return head_word(LIGHT_HEAD_HITS);
  }
  return eye_word(GRID + GRID_HITS_AT);
}

fn hits_room() -> u32 {
  return select(HITS, LIGHT_PARKED_ROOM, light_run());
}

/// The holes the queue takes: the light's slots its cast listed (`light::misses_at`); the
/// eye's walked pixels its cast listed, or, where the eye's queue runs alone, the pixels the
/// queue before left parked (walked.wgsl `holes_list_at`, pxl-engine `ray::holes_from_at`).
fn item_count() -> u32 {
  if (light_run()) {
    return in_room(beams[head_word(LIGHT_HEAD_MISSES_CNT)], room());
  }
  return in_room(beams[eye_holes_at()], room());
}

/// The `k`th hole the queue takes.
fn item_at(k: u32) -> u32 {
  if (light_run()) {
    return beams[head_word(LIGHT_HEAD_MISSES) + k];
  }
  return beams[eye_holes_at() + WALKED_LIST + k];
}

/// The first word, its count, of the list of the eye's holes the queue takes this frame.
fn eye_holes_at() -> u32 {
  let drawn = room();
  return holes_list_at(drawn, beams[holes_from_at(drawn)]);
}

/// The first word, its count, of the eye's owed list the queue lists the pixels it leaves
/// parked in: the one it does not take.
fn eye_owed_at() -> u32 {
  let drawn = room();
  return owed_into_at(drawn, beams[holes_from_at(drawn)]);
}

/// The first word of a hole's state: the eye's at its pixel's index in the region past its
/// list's words (pxl-engine `ray::region_words`); a light's slot's at its tile's place and its
/// slot within the tile, in its cast's region (light_aim.wgsl `light_state_at`).
fn state_at(hole: u32) -> u32 {
  let at = hole_at(hole);
  if (light_run()) {
    let place = beams[light_place_at(at)];
    let k = (at.y % LIGHT_TILE) * LIGHT_TILE + at.x % LIGHT_TILE;
    return light_state_at(place, k, head_word(LIGHT_HEAD_CAST));
  }
  return eye_state_at(at, textureDimensions(rays).xy, arrayLength(&beams));
}

/// A hole's ray.
fn item_aim(hole: u32) -> Aim {
  let at = hole_at(hole);
  if (light_run()) {
    return light_aim(slot_texel(at));
  }
  return px_aim(at, textureDimensions(rays));
}

/// Where a hole's ray starts afresh: its cell's start.
fn item_start(hole: u32, aim: Aim) -> Start {
  let at = hole_at(hole);
  if (light_run()) {
    return light_resumed(aim, slot_texel(at));
  }
  return px_start(aim, at, textureDimensions(rays));
}

/// A hole's ray parked at its node `corner`, `zoom`, `roots`, `t` along it, `step` steps taken,
/// at its state (parked.wgsl).
fn park(hole: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) {
  let at = state_at(hole);
  beams[at] = bitcast<u32>(t);
  beams[at + 1u] = corner.x;
  beams[at + 2u] = corner.y;
  beams[at + 3u] = corner.z;
  beams[at + 4u] = parked_word(zoom, roots, step);
}

/// Whether a hole's ray is parked.
fn parked(hole: u32) -> bool {
  return (beams[state_at(hole) + 4u] & PARKED) != 0u;
}

/// What a hole's first walk of a run takes: the state it is parked in, a light's slot of an
/// owed tile, whose claim took its state (light_propagate.wgsl `light_claim_cs`), or an eye's
/// pixel whose rays stand, which its cast or the frame's queue alone left parked (cast.wgsl,
/// pxl-engine `node_cache::RESUMES`); else a fresh start. The second walk takes those the first
/// parked.
fn taken(hole: u32, again: bool) -> u32 {
  if (parked(hole)) {
    return TAKEN_PARKED;
  }
  return select(TAKEN_FRESH, TAKEN_NONE, again);
}

/// The state a hole's ray is parked in, where a walk goes on from.
fn parked_start(hole: u32) -> Start {
  let at = state_at(hole);
  return parked_from(beams[at], vec3<u32>(beams[at + 1u], beams[at + 2u], beams[at + 3u]), beams[at + 4u]);
}

/// A hole's ray no longer parked.
fn unpark(hole: u32) {
  beams[state_at(hole) + 4u] = 0u;
}

/// A stop listed for the record, what lies about its face put at its pixel off it (miss_word.wgsl
/// `hit_put`, record_stops.wgsl): numbered as the walk that ended it numbers them
/// (`stop_number`), the dispatch over them raised by the number that opens a workgroup, and its
/// number marked waiting (`beam::HIT_WAITS`); none past its observer's room (`hits_room`).
fn keep_hit(w: Walked, face: u32, id: vec2<u32>) -> u32 {
  let r = stop_number();
  if (r >= hits_room()) {
    return 0u;
  }
  if (r % HIT_LANES == 0u) {
    stop_rows(workgroups(r + 1u, hits_room(), HIT_LANES));
  }
  hit_put(r, w, face, id);
  return HIT_WAITS | r;
}

/// Hole `hole`'s walk `w`, ended, its words written and its stop, whatever it stopped on,
/// listed for its observer's record (`keep_hit`, record_stops.wgsl), the eye's and the
/// light's alike: a slot's depth; the eye's ray's, what lies about the face it stopped on its
/// box's for a cell short of its cube (pxl-engine `face::BOX`), a lack's none, a whole cell's
/// put there off the record once its stop is in (record_stops.wgsl `record_faces_cs`). A
/// parked ray keeps no stop until it ends.
fn finish(hole: u32, aim: Aim, w: Walked) {
  unpark(hole);
  let at = hole_at(hole);
  let face = face_of(aim, w);
  if (light_run()) {
    if (w.ends == 1u || w.ends == 2u) {
      _ = keep_hit(w, face | select(0u, HIT_LACKS, w.ends == 2u), at);
    }
    textureStore(rays, at, 0u, vec4<u32>(walked_word(w), 0u, 0u, 0u));
    return;
  }
  var faces = 0u;
  // Both of a `select` are taken, so the stop is kept under an `if`.
  if (w.ends == 1u || w.ends == 2u) {
    faces = keep_hit(w, face | select(0u, HIT_LACKS, w.ends == 2u), at);
    if (w.ends == 1u && hit_fill(w) != 0u) {
      faces = FACE_BOX;
    }
  }
  write_ray(at, ray_word(w, face, faces));
}

/// A hole's ray the frame's budget does not reach, at the node `corner`, `zoom`, `roots`, `t`
/// along it, `step` steps taken: parked where its state lies, its word left as the cast wrote
/// it, and counted as its observer's owed work (`node_cache::OWED_AT`), which the renderer
/// reads back and wakes the next frame by. An eye's pixel keeps what the record drew there
/// (cast.wgsl), and the frame's queue alone takes it on while the rays stand. A light's slot's
/// tile is owed (pxl-engine `light::READ_OWED`), so the next claim takes it nearest first and
/// the ray goes on from there; its word written when it ends, which that cast's lay of the
/// tiles read takes (gpu/shadow.rs `begin`). The tile's read word is the claim's and the
/// slide's to write but for this bit, which every slot of the tile it sets sets alike.
fn unreached(hole: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) {
  park(hole, t, corner, zoom, roots, step);
  if (!light_run()) {
    // Listed for the queue that goes on next, at its number among the owed, which the host
    // copies in as that list's count (pxl-engine `ray::owed_list_at`). A hole is taken once a
    // walk, so the list holds each parked pixel once.
    let r = atomicAdd(&node_cache[EYE_OWED], 1u);
    if (r < room()) {
      beams[eye_owed_at() + WALKED_LIST + r] = hole;
    }
    return;
  }
  let tile = slot_texel(hole_at(hole)) / LIGHT_TILE;
  let w = head_word(LIGHT_HEAD_READS) + tile.y * head_word(LIGHT_HEAD_TILES) + tile.x;
  if (w < arrayLength(&beams)) {
    beams[w] = beams[w] | LIGHT_READ_OWED;
  }
  atomicAdd(&node_cache[LIGHT_OWED], 1u);
}
