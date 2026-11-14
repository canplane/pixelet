// pxl-render/frame/shader/walk_word.wgsl

// The words the walks read. Their constants build.rs writes after these from the pxl-engine
// layouts that define them, and with them `is_matter` from pxl-spec `cube::is_matter`.

// Where a node cache slot's words lie. Every pass over the node cache addresses a slot through
// these, and each matches the pxl-engine `node_cache` function of the same name.

fn entry_at(slot: u32) -> u32 {
  return ENTRIES_AT + slot * ENTRY_WORDS;
}

fn key_at(slot: u32) -> u32 {
  return KEYS_AT + slot;
}

fn state_at(slot: u32) -> u32 {
  return STATES_AT + slot;
}

fn drawn_at(slot: u32) -> u32 {
  return RECS_AT + slot * R_WORDS + R_DRAWN;
}

fn face_at(slot: u32, face: u32) -> u32 {
  return RECS_AT + slot * R_WORDS + R_FACES + face;
}

// How a node's index is wrapped and counted, as pxl-spec space/coord states it: X wrapped
// about the world, and Y and Z turned into counts from its edge.

/// The port of pxl-spec `wrap_idx`.
fn wrap_idx(zoom: u32, x: i32) -> i32 {
  if (zoom == 0u) {
    return 0i;
  }
  return (x << (32u - zoom)) >> (32u - zoom);
}

/// The port of pxl-spec `zoom::y_start`.
fn y_start(zoom: u32) -> i32 {
  return select(i32(1u << ((zoom - 1u) & 31u)), 0i, zoom == 0u);
}

/// The port of pxl-spec `zoom::z_start`.
fn z_start(zoom: u32) -> i32 {
  return select(i32(1u << ((zoom - SHARD_ZOOM - 1u) & 31u)), 0i, zoom <= SHARD_ZOOM);
}

/// The port of pxl-engine `object::path`.
fn path(zoom: u32, at: vec3<i32>) -> vec3<i32> {
  return vec3<i32>(at.x & i32((1u << zoom) - 1u), at.y + y_start(zoom), at.z + z_start(zoom));
}
