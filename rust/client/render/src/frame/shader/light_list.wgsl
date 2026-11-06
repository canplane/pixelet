// pxl-render/light_list.wgsl

// The shadow map's lattice as the light's beam, cells and slots take it off its list
// (pxl-engine light.rs): where a beam ray's line and start and a sample lie (pxl-engine light.rs), off the words the head lays (`light::head_layout`); which
// place of the lattice the thread at an id takes (`light_item`); and the reads and each list's
// marks, as the receivers' passes claim tiles (`light::reads_at`, `listed_marks_at`). The list
// is `beams`, as resume.wgsl names it; the receivers' passes bind
// it apart (light_stand.wgsl).

/// The levels of the map's lattice: a beam ray's corner, a cell, a sample, a slot's ray.
const LEVEL_CORNER = 0u;
const LEVEL_CELL = 1u;
const LEVEL_SAMPLE = 2u;
const LEVEL_RAY = 3u;

/// The beam rays across the map, and down (`light::beam_across`).
fn beam_across() -> u32 {
  return head_word(LIGHT_HEAD_BEAM_ACROSS);
}

/// The first word of beam ray `k`'s line (`light::line`).
fn light_line(k: u32) -> u32 {
  return k * LIGHT_LINE_WORDS;
}

/// The word of how far the rays about beam ray `k` start (`light::hand_at`): a word a beam
/// ray, past the lines, before the cells.
fn light_hand_at(k: u32) -> u32 {
  return head_word(LIGHT_HEAD_HANDS) + k;
}

/// The first word of sample `s`, wrapped as the slots are (`light::sample`).
fn light_sample_at(s: vec2<u32>) -> u32 {
  let n = head_word(LIGHT_HEAD_SAMPLES_ACROSS);
  return head_word(LIGHT_HEAD_SAMPLES) + ((s.y % n) * n + s.x % n) * SAMPLE_WORDS;
}

/// List `which`'s count, its entries after it (`light::listed_at`).
fn listed_at(which: u32) -> u32 {
  return head_word(LIGHT_HEAD_LISTED + which);
}

/// The places of list `which`: every beam ray's corner, or every tile (`light::listed_room`).
fn listed_room(which: u32) -> u32 {
  if (which == LIGHT_LISTED_CORNERS) {
    return beam_across() * beam_across();
  }
  return light_tiles() * light_tiles();
}

/// The list what feeds a claimed tile at `level` is listed in (`light::LISTED_CORNERS`,
/// `LISTED_FED`, `LISTED_CLAIMED`).
fn listed(level: u32) -> u32 {
  if (level == LEVEL_CORNER) {
    return LIGHT_LISTED_CORNERS;
  }
  if (level == LEVEL_RAY) {
    return LIGHT_LISTED_CLAIMED;
  }
  return LIGHT_LISTED_FED;
}

/// The slot box texel `p` lies in: light_aim.wgsl's `slot_texel` the other way.
fn texel_slot(p: vec2<u32>) -> vec2<u32> {
  return vec2<u32>(vec2<i32>(p) + light_corner()) & vec2<u32>(head_word(LIGHT_HEAD_SIZE) - 1u);
}

/// What of `level` the thread at `id` of its pass takes: off the list of what feeds the tiles
/// the cast claimed, a workgroup of
/// `SAMPLE_SIDE` × `SAMPLE_SIDE` `LISTED_LANES` and an entry its side's square of them; none past
/// the list.
fn light_item(level: u32, id: vec2<u32>) -> vec2<u32> {
  let place = ((id.y / SAMPLE_SIDE) * LIGHT_LIST_ACROSS + id.x / SAMPLE_SIDE) * LIGHT_LISTED_LANES
    + (id.y % SAMPLE_SIDE) * SAMPLE_SIDE + id.x % SAMPLE_SIDE;
  var across = 1u;
  if (level == LEVEL_SAMPLE) {
    across = LIGHT_TILE / LIGHT_SAMPLE;
  } else if (level == LEVEL_RAY) {
    across = LIGHT_TILE;
  }
  let per = across * across;
  let e = place / per;
  let k = place % per;
  let which = listed(level);
  let at = listed_at(which);
  if (e >= in_room(beams[at], listed_room(which))) {
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
  return head_word(LIGHT_HEAD_TILES);
}

/// The reads, a word a tile (`light::reads_at`).
fn reads_at() -> u32 {
  return head_word(LIGHT_HEAD_READS);
}

/// The first of list `which`'s marks among the args' (light_propagate.wgsl), a word a place
/// (`light::listed_marks_at`).
fn listed_marks_at(which: u32) -> u32 {
  return head_word(LIGHT_HEAD_MARKS + which);
}
