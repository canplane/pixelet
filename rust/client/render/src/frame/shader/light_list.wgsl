// pxl-render/light_list.wgsl

// The shadow map's lattice, as the light's beam, cells and slots read it from the light's list
// (pxl-engine light.rs). This part says where a beam ray's line, how far the rays about it
// start, a cell's start and a sample lie, from the words the head lays out
// (`light::header_layout`). It says which place of the lattice the thread at an id takes
// (`light_item`). It also says where the reads and each list's marks lie, by which the
// receivers' passes claim tiles (`light::reads_at`, `list_marks_at`). The list is bound as
// `beams`, the name resume.wgsl reads it by. The receivers' passes bind it under a binding of
// their own (light_stand.wgsl).

/// The levels of the map's lattice, coarse to fine: a beam ray's corner, a cell, a sample, and
/// a slot's ray.
const LEVEL_CORNER = 0u;
const LEVEL_CELL = 1u;
const LEVEL_SAMPLE = 2u;
const LEVEL_RAY = 3u;

/// The number of beam rays across the map, which is also the number down.
fn beam_across() -> u32 {
  return header_word(LIGHT_HEADER_BEAM_ACROSS);
}

fn light_line(k: u32) -> u32 {
  return k * LIGHT_LINE_WORDS;
}

/// The word that says how far the rays about beam ray `k` start (`light::hand_at`). There is
/// one word for each beam ray, past the lines and before the cells.
fn light_hand_at(k: u32) -> u32 {
  return header_word(LIGHT_HEADER_HANDS) + k;
}

/// The first word of sample `s`, wrapped as the slots are (`light::sample`).
fn light_sample_at(s: vec2<u32>) -> u32 {
  let n = header_word(LIGHT_HEADER_SAMPLES_ACROSS);
  return header_word(LIGHT_HEADER_SAMPLES) + ((s.y % n) * n + s.x % n) * SAMPLE_WORDS;
}

/// Where list `which` lies: its count, then its entries (`light::list_cnt_at`).
fn list_cnt_at(which: u32) -> u32 {
  return header_word(LIGHT_HEADER_LISTS + which);
}

/// The number of places in list `which`: one for every beam ray's corner, or one for every
/// tile (`light::list_cap`).
fn list_cap(which: u32) -> u32 {
  if (which == LIGHT_LIST_CORNERS) {
    return beam_across() * beam_across();
  }
  return light_tiles() * light_tiles();
}

/// The list in which what feeds a claimed tile at `level` is listed (`light::LIST_CORNERS`,
/// `LIST_SOURCES`, `LIST_DAMAGE`). Cells and samples share the sources' list.
fn list_of(level: u32) -> u32 {
  if (level == LEVEL_CORNER) {
    return LIGHT_LIST_CORNERS;
  }
  if (level == LEVEL_RAY) {
    return LIGHT_LIST_DAMAGE;
  }
  return LIGHT_LIST_SOURCES;
}

/// The slot that holds box texel `p`, the inverse of light_aim.wgsl `slot_texel`.
fn texel_slot(p: vec2<u32>) -> vec2<u32> {
  return vec2<u32>(vec2<i32>(p) + light_corner()) & vec2<u32>(header_word(LIGHT_HEADER_SIZE) - 1u);
}

/// What of `level` the thread at `id` of its pass takes, from the list of what feeds the tiles
/// the cast claimed. A workgroup is a square of `SAMPLE_SIDE` × `SAMPLE_SIDE` threads, the
/// list's `LIST_LANES`, and an entry takes a square of places (`light_item_at`). Past the list
/// the thread takes `NONE`.
fn light_item(level: u32, id: vec2<u32>) -> vec2<u32> {
  let place = ((id.y / SAMPLE_SIDE) * LIGHT_LIST_ACROSS + id.x / SAMPLE_SIDE) * LIGHT_LIST_LANES
    + (id.y % SAMPLE_SIDE) * SAMPLE_SIDE + id.x % SAMPLE_SIDE;
  return light_item_at(level, place);
}

/// What of `level` lies at place `place` of the list of what feeds the tiles the cast claimed.
/// Each entry takes `across` × `across` places in order, where `across` is how many of the
/// level's items lie along a tile's side. Past the list it is `NONE`.
fn light_item_at(level: u32, place: u32) -> vec2<u32> {
  var across = 1u;
  if (level == LEVEL_SAMPLE) {
    across = LIGHT_TILE / LIGHT_SAMPLE;
  } else if (level == LEVEL_RAY) {
    across = LIGHT_TILE;
  }
  let per = across * across;
  let e = place / per;
  let k = place % per;
  let which = list_of(level);
  let at = list_cnt_at(which);
  if (e >= in_cap(beams[at], list_cap(which))) {
    return vec2<u32>(NONE);
  }
  let w = beams[at + 1u + e];
  let t = vec2<u32>(w & 0xffffu, w >> 16u);
  if (level == LEVEL_CORNER || level == LEVEL_CELL) {
    return t;
  }
  let within = vec2<u32>(k % across, k / across);
  let p = texel_slot(t * LIGHT_TILE);
  if (level == LEVEL_SAMPLE) {
    return p / LIGHT_SAMPLE + within;
  }
  return p + within;
}

/// The box texel that beam ray `c` starts over. Beam ray 0 lies `BEAM_WINDOW` beam rays before
/// the box's first texel (`light::corner_at`).
fn corner_texel(c: vec2<u32>) -> vec2<i32> {
  return (vec2<i32>(c) - vec2<i32>(i32(LIGHT_BEAM_WINDOW))) * i32(LIGHT_BEAM);
}

/// The box tile on which the item of `level` at `at` lies. A cell is numbered by its tile. A
/// beam ray lies on the tile of its texel, which may lie past the box.
fn light_on(level: u32, at: vec2<u32>) -> vec2<i32> {
  if (level == LEVEL_CELL) {
    return vec2<i32>(at);
  }
  if (level == LEVEL_CORNER) {
    let p = corner_texel(at);
    let t = i32(LIGHT_TILE);
    return select(p / t, (p - vec2<i32>(t - 1i)) / t, p < vec2<i32>(0));
  }
  var p = at;
  if (level == LEVEL_SAMPLE) {
    p = at * LIGHT_SAMPLE;
  }
  return vec2<i32>(slot_texel(p) / LIGHT_TILE);
}

/// The number of tiles across the map, which is also the number down.
fn light_tiles() -> u32 {
  return header_word(LIGHT_HEADER_TILES);
}

/// Where the reads lie: one word for each tile, holding the number of the cast that last
/// claimed it (`light::reads_at`).
fn reads_at() -> u32 {
  return header_word(LIGHT_HEADER_READS);
}

/// Where the marks of list `which` begin among the args' words (light_propagate.wgsl). Each
/// place has a word holding the last cast that listed it (`light::list_marks_at`).
fn list_marks_at(which: u32) -> u32 {
  return header_word(LIGHT_HEADER_MARKS + which);
}
