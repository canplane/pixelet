// pxl-render/light_stand.wgsl

// The light's list (pxl-engine light.rs `list_words`) as the passes that stand, read and claim
// its tiles write it, bound apart from `beams` (light_list.wgsl): a word read or written whole,
// none past the buffer. No word of it atomic: a listed tile's place is its own, off a count
// kept atomic apart (light_receiver_tiles.wgsl, light_propagate.wgsl); a read is the word of the one
// thread that listed its tile read, a square's the cast's number whichever of its tiles sets
// it; and the slide's words are each one thread's (`light::slide_at`).

@group(0) @binding(12) var<storage, read_write> list: array<u32>;
/// The list as light_list.wgsl's readers name it, which these passes take none of: bound
/// where the beam's and the slots' passes bind it, and read by no entry here.
@group(0) @binding(8) var<storage, read> beams: array<u32>;

fn list_at(i: u32) -> u32 {
  if (i < arrayLength(&list)) {
    return list[i];
  }
  return 0u;
}

fn list_put(i: u32, v: u32) {
  if (i < arrayLength(&list)) {
    list[i] = v;
  }
}

/// No tile's place (`tile_place`).
const NO_TILE: u32 = 0xffffffffu;

/// Tile `t`'s place among the box's tiles, row by row, `NO_TILE` past the box.
fn tile_place(t: vec2<i32>) -> u32 {
  let n = i32(light_tiles());
  if (all(t >= vec2<i32>(0)) && all(t < vec2<i32>(n))) {
    return u32(t.y * n + t.x);
  }
  return NO_TILE;
}

/// The first cast whose reads hold: the later of the host's epoch and the one a slide unrun
/// began (`light::reads_epoch`, `light::READS_FROM`).
fn reads_epoch() -> u32 {
  return max(light_epoch(), list_at(header_word(LIGHT_HEADER_SLIDE) + LIGHT_READS_FROM));
}
