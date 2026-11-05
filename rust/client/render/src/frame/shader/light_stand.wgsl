// pxl-render/light_stand.wgsl

// The light's list (pxl-engine light.rs `list_words`) as the passes that mark, read and claim
// its tiles write it, bound apart from `beams` (light_list.wgsl): a word read or written whole,
// none past the buffer; and the tiles a receiver stands on (`light::stands_at`). No word of it
// atomic: a stand is one number, the cast's, whichever receivers set it; a read is its tile's
// thread's alone, a square's the cast's number whichever of its tiles sets it, and a listed
// tile's place its own, off the count in the args (light_propagate.wgsl).

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

/// No tile's word (`tile_word`).
const NO_TILE: u32 = 0xffffffffu;

/// Tile `t`'s word among the box's tiles' words at `first`, `NO_TILE` past the box.
fn tile_word(first: u32, t: vec2<i32>) -> u32 {
  let n = i32(light_tiles());
  if (all(t >= vec2<i32>(0)) && all(t < vec2<i32>(n))) {
    return first + u32(t.y * n + t.x);
  }
  return NO_TILE;
}

/// Tile `t` marked stood on in the cast numbered `current`, none past the box: written
/// only where it does not hold that number yet (pxl-engine `light::stands_at`).
fn stand(current: u32, t: vec2<i32>) {
  let w = tile_word(stands_at(), t);
  if (w != NO_TILE && list_at(w) != current) {
    list_put(w, current);
  }
}

/// Whether a receiver stood on tile `t` in the cast numbered `current`, none past the box.
fn stood(current: u32, t: vec2<i32>) -> bool {
  let w = tile_word(stands_at(), t);
  return w != NO_TILE && list_at(w) == current;
}
