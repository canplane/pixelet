// pxl-render/queue.wgsl

// The holes' rays, an observer's: the pixels the eye's cast marked and the slots the light's
// listed, which no voxel answers (cast.wgsl, light.wgsl), walked off one queue by lanes that
// each take the next ray when theirs ends or is parked (queue_walk.wgsl, queue_last.wgsl;
// pxl-engine `ray::QUEUE_LANES`). The observer is the head the pass binds (light_aim.wgsl
// `light_run`): one code for both, its pixel or slot, its ray, its start and its words chosen
// by it. A ray is parked as its walk's state (`Walked`, walk.wgsl) at its hole's place in the
// holes' region, the last words of the observer's list (`region_at`), from which a later walk
// goes on as one unbroken walk; the region holds a state for every hole a cast can list, so
// every ray can be parked.

/// The region's first word, sized to `room` holes and `lists` lists (pxl-engine
/// `ray::region_words`), the last words of the observer's list.
fn region_at(room: u32, lists: u32) -> u32 {
  return arrayLength(&beams) - (lists * (1u + room) + room * STATE_WORDS);
}

/// A hole's pixel or slot, its column under its row.
fn hole_at(hole: u32) -> vec2<u32> {
  return vec2<u32>(hole & 0xffffu, hole >> 16u);
}

/// The holes a cast can list (pxl-engine `ray::region_words`): the eye's drawn pixels, the
/// light's slots.
fn room() -> u32 {
  if (light_run()) {
    let size = head_word(LIGHT_HEAD_SIZE);
    return size * size;
  }
  let size = textureDimensions(rays);
  return size.x * size.y;
}

/// The region's lists: the light's carried slots, the one a cast reads, then the one it writes
/// by its number's parity (`carried_in`, `carried_out`); the eye's none, its holes listed apart
/// (walked.wgsl).
fn lists() -> u32 {
  return select(0u, CARRIED_LISTS, light_run());
}

/// The first word of list `i`, its count.
fn list_at(i: u32) -> u32 {
  return region_at(room(), lists()) + i * (1u + room());
}

fn carried_in() -> u32 {
  return list_at(1u - head_word(LIGHT_HEAD_CAST) % 2u);
}

fn carried_out() -> u32 {
  return list_at(head_word(LIGHT_HEAD_CAST) % 2u);
}

/// The slots the light's cast before carried, the first the queue takes; the eye none.
fn carried() -> u32 {
  if (light_run()) {
    return in_room(beams[carried_in()], room());
  }
  return 0u;
}

/// The holes the observer's cast listed: the light's slots (`light::misses_at`), the eye's
/// walked pixels (walked.wgsl).
fn listed() -> u32 {
  if (light_run()) {
    return in_room(beams[head_word(LIGHT_HEAD_MISSES_CNT)], room());
  }
  return in_room(beams[walked_cnt_at()], room());
}

/// The `k`th hole the queue takes: the carried, then the listed.
fn queued(k: u32) -> u32 {
  let c = carried();
  if (k < c) {
    return beams[carried_in() + 1u + k];
  }
  if (light_run()) {
    return beams[head_word(LIGHT_HEAD_MISSES) + k - c];
  }
  return beams[walked_list_at(k - c)];
}

/// The first word of a hole's state, at its pixel's or slot's index.
fn state_at(hole: u32) -> u32 {
  let at = hole_at(hole);
  var across = textureDimensions(rays).x;
  if (light_run()) {
    across = head_word(LIGHT_HEAD_SIZE);
  }
  return region_at(room(), lists()) + lists() * (1u + room()) + (at.y * across + at.x) * STATE_WORDS;
}

/// A hole's ray.
fn hole_aim(hole: u32) -> Aim {
  let at = hole_at(hole);
  if (light_run()) {
    return light_aim(slot_texel(at));
  }
  return px_aim(at, textureDimensions(rays));
}

/// Where a hole's ray starts afresh: its cell's start.
fn hole_start(hole: u32, aim: Aim) -> Start {
  let at = hole_at(hole);
  if (light_run()) {
    return light_resumed(aim, slot_texel(at));
  }
  return px_start(aim, at, textureDimensions(rays));
}

/// A hole's ray parked at its node `corner`, `zoom`, `roots`, `t` along it, `step` steps taken:
/// its distance, its node's corner, and its zoom, roots and steps over `ZOOM_BITS`, three bits a
/// root and the rest, under `PARKED` (pxl-engine `ray::STATE_WORDS`).
fn park(hole: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) {
  let at = state_at(hole);
  beams[at] = bitcast<u32>(t);
  beams[at + 1u] = corner.x;
  beams[at + 2u] = corner.y;
  beams[at + 3u] = corner.z;
  let r = vec3<u32>(roots + vec3<i32>(3));
  beams[at + 4u] = PARKED | zoom | (r.x << 6u) | (r.y << 9u) | (r.z << 12u) | (step << 15u);
}

/// Whether a hole's ray is parked.
fn parked(hole: u32) -> bool {
  return (beams[state_at(hole) + 4u] & PARKED) != 0u;
}

/// The state a hole's ray is parked in, where a walk goes on from: its node asked from the top,
/// as no parent's handed words are parked.
fn parked_start(hole: u32) -> Start {
  let at = state_at(hole);
  let p = beams[at + 4u];
  let r = vec3<i32>(vec3<u32>(p >> 6u, p >> 9u, p >> 12u) & vec3<u32>(7u)) - vec3<i32>(3);
  return Start(
    bitcast<f32>(beams[at]),
    vec3<u32>(beams[at + 1u], beams[at + 2u], beams[at + 3u]),
    p & 63u,
    r,
    vec4<u32>(0u),
    true,
    (p & ~PARKED) >> 15u,
  );
}

/// A hole's ray no longer parked.
fn unpark(hole: u32) {
  beams[state_at(hole) + 4u] = 0u;
}

/// Hole `hole`'s walk `w`, ended, its words written: a slot's depth; the eye's ray's, and its
/// stop, whatever it stopped on, listed for the record (`keep_hit`, record_stops.wgsl): what
/// lies about the face it stopped on its box's for a cell short of its cube (pxl-engine
/// `face::BOX`), a lack's none, a whole cell's put there off the record once its stop is in
/// (record_stops.wgsl `record_faces_cs`).
fn finish(hole: u32, aim: Aim, w: Walked) {
  unpark(hole);
  let at = hole_at(hole);
  if (light_run()) {
    textureStore(rays, at, 0u, vec4<u32>(walked_word(w), 0u, 0u, 0u));
    return;
  }
  let face = face_of(aim, w);
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

/// The observer's lanes of the nodes its holes' walks visited and asked (pxl-engine
/// `count::MISS`, `count::ASKED`).
fn visits_at() -> u32 {
  return select(EYE_MISS_COUNT, LIGHT_MISS_COUNT, light_run());
}

fn asked_at() -> u32 {
  return select(EYE_ASKED_COUNT, LIGHT_ASKED_COUNT, light_run());
}
