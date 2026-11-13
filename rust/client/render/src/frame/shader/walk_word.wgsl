// pxl-render/frame/shader/walk_word.wgsl

// The words the walks read. Their constants build.rs writes after these from the pxl-engine
// layouts that define them.

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
