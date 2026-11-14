// pxl-render/frame/shader/hole.light.wgsl

// The light's holes as the holes' queue reads them (queue.wgsl): the slots the light's cast
// listed (light.wgsl), where its pool and hits lie, a slot's ray, and the word an ended walk
// leaves at the slot.

fn pool_at() -> u32 {
  return hdr_word(LIGHT_HDR_POOL);
}

/// Where the light's hits lie (hit.wgsl), past its pool (`light::hits_at`).
fn hits_at() -> u32 {
  return hdr_word(LIGHT_HDR_HITS);
}

/// Where the listed holes lie: their count, then their entries (`light::holes_at`). There is
/// room for one a slot of the map.
fn holes_at() -> u32 {
  return hdr_word(LIGHT_HDR_HOLES);
}

fn holes_cap() -> u32 {
  let size = hdr_word(LIGHT_HDR_SIZE);
  return size * size;
}

fn item_aim(item: u32) -> Aim {
  return light_aim(slot_texel(hole_px(item)));
}

/// Where a hole's ray starts afresh: at the start of its cell.
fn item_start(item: u32, aim: Aim) -> Start {
  return light_resumed(aim, slot_texel(hole_px(item)));
}

/// The light's hits hold no line: the light asks no face's word off one
/// (record_hits.light.wgsl `hit_asks`).
fn hit_line(id: vec2<u32>) -> u32 {
  return NONE;
}

/// Writes at slot `at` the depth of what walk `w` shows, `shown`.
fn hole_write(at: vec2<u32>, w: WalkEnd, shown: WalkEnd, face: u32, faces: u32) {
  textureStore(rays, at, 0u, vec4<u32>(walked_word(shown), 0u, 0u, 0u));
}
