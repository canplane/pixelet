// pxl-render/light_list.wgsl

// The shadow map's lattice as the light's beam, cells and slots take it off its list
// (pxl-engine light.rs): where a beam ray's line and start and a sample lie (pxl-engine light.rs), off the words the head lays (`light::header_layout`); which
// place of the lattice the thread at an id takes (`light_item`); and the reads and each list's
// marks, as the receivers' passes claim tiles (`light::reads_at`, `list_marks_at`). The list
// is `beams`, as resume.wgsl names it; the receivers' passes bind
// it apart (light_stand.wgsl).

/// The levels of the map's lattice: a beam ray's corner, a cell, a sample, a slot's ray.
const LEVEL_CORNER = 0u;
const LEVEL_CELL = 1u;
const LEVEL_SAMPLE = 2u;
const LEVEL_RAY = 3u;

/// The beam rays across the map, and down (`light::beam_across`).
fn beam_across() -> u32 {
  return header_word(LIGHT_HEADER_BEAM_ACROSS);
}

/// The first word of beam ray `k`'s line (`light::line`).
fn light_line(k: u32) -> u32 {
  return k * LIGHT_LINE_WORDS;
}

/// The word of how far the rays about beam ray `k` start (`light::hand_at`): a word a beam
/// ray, past the lines, before the cells.
fn light_hand_at(k: u32) -> u32 {
  return header_word(LIGHT_HEADER_HANDS) + k;
}

/// The first word of sample `s`, wrapped as the slots are (`light::sample`).
fn light_sample_at(s: vec2<u32>) -> u32 {
  let n = header_word(LIGHT_HEADER_SAMPLES_ACROSS);
  return header_word(LIGHT_HEADER_SAMPLES) + ((s.y % n) * n + s.x % n) * SAMPLE_WORDS;
}

/// List `which`'s count, its entries after it (`light::list_cnt_at`).
fn list_cnt_at(which: u32) -> u32 {
  return header_word(LIGHT_HEADER_LISTS + which);
}

/// The places of list `which`: every beam ray's corner, or every tile (`light::list_cap`).
fn list_cap(which: u32) -> u32 {
  if (which == LIGHT_LIST_CORNERS) {
    return beam_across() * beam_across();
  }
  return light_tiles() * light_tiles();
}

/// The list what feeds a claimed tile at `level` is listed in (`light::LIST_CORNERS`,
/// `LIST_SOURCES`, `LIST_DAMAGE`).
fn list_of(level: u32) -> u32 {
  if (level == LEVEL_CORNER) {
    return LIGHT_LIST_CORNERS;
  }
  if (level == LEVEL_RAY) {
    return LIGHT_LIST_DAMAGE;
  }
  return LIGHT_LIST_SOURCES;
}

/// The slot box texel `p` lies in: light_aim.wgsl's `slot_texel` the other way.
fn texel_slot(p: vec2<u32>) -> vec2<u32> {
  return vec2<u32>(vec2<i32>(p) + light_corner()) & vec2<u32>(header_word(LIGHT_HEADER_SIZE) - 1u);
}

/// What of `level` the thread at `id` of its pass takes: off the list of what feeds the tiles
/// the cast claimed, a workgroup of
/// `SAMPLE_SIDE` × `SAMPLE_SIDE` `LIST_LANES` and an entry its side's square of them; none past
/// the list.
fn light_item(level: u32, id: vec2<u32>) -> vec2<u32> {
  let place = ((id.y / SAMPLE_SIDE) * LIGHT_LIST_ACROSS + id.x / SAMPLE_SIDE) * LIGHT_LIST_LANES
    + (id.y % SAMPLE_SIDE) * SAMPLE_SIDE + id.x % SAMPLE_SIDE;
  return light_item_at(level, place);
}

/// What of `level` lies at place `place` of the list of what feeds the tiles the cast claimed,
/// an entry its side's square of places in order; none past the list.
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

/// The box texel beam ray `c` starts over, `BEAM_WINDOW` beam rays before the box's first
/// (`light::corner_at`).
fn corner_texel(c: vec2<u32>) -> vec2<i32> {
  return (vec2<i32>(c) - vec2<i32>(i32(LIGHT_BEAM_WINDOW))) * i32(LIGHT_BEAM);
}

/// The tile of the box what `level` at `at` lies on: a cell's its own number, a
/// beam ray's the tile its texel, which may lie past the box, lies on.
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

/// The tiles across the map, and down (`light::tiles`).
fn light_tiles() -> u32 {
  return header_word(LIGHT_HEADER_TILES);
}

/// The reads, a word a tile (`light::reads_at`).
fn reads_at() -> u32 {
  return header_word(LIGHT_HEADER_READS);
}

/// The first of list `which`'s marks among the args' (light_propagate.wgsl), a word a place
/// (`light::list_marks_at`).
fn list_marks_at(which: u32) -> u32 {
  return header_word(LIGHT_HEADER_MARKS + which);
}
