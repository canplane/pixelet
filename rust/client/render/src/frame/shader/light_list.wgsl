// pxl-render/light_list.wgsl

// The shadow map's lattice as the light's beam, cells and slots take it off its list
// (pxl-shader light.rs `Map`): where a beam ray's line and start, a cell's start and a
// sample lie (pxl-engine light.rs), off the words the head lays (`light::head_layout`); which
// place of the lattice the thread at an id takes (`Map::take`); and whether the cast needs it
// (`Map::needs`). The list is `beams`, as resume.wgsl names it.

/// The levels of the map's lattice (pxl-shader observer.rs `Level`).
const LEVEL_CORNER = 0u;
const LEVEL_CELL = 1u;
const LEVEL_SAMPLE = 2u;
const LEVEL_RAY = 3u;

/// The beam rays across the map, and down (`light::beam_across`).
fn beam_across() -> u32 {
  return head_word(LIGHT_HEAD_SIZE) / LIGHT_BEAM + 1u + 2u * LIGHT_BEAM_WINDOW;
}

/// The first word of beam ray `k`'s line (`light::line`).
fn light_line(k: u32) -> u32 {
  return k * LIGHT_LINE_WORDS;
}

/// The word of how far the rays about beam ray `k` start (`light::hand_at`): a word a beam
/// ray, past the lines, before the cells.
fn light_hand_at(k: u32) -> u32 {
  let n = beam_across();
  return head_word(LIGHT_HEAD_CELLS) - n * n + k;
}

/// The first word of cell `c`'s start (`light::cell`).
fn light_cell_at(c: vec2<u32>) -> u32 {
  return head_word(LIGHT_HEAD_CELLS) + (c.y * head_word(LIGHT_HEAD_CELLS_ACROSS) + c.x) * CELL_WORDS;
}

/// The first word of sample `s`, wrapped as the slots are (`Map::sample_at`).
fn light_sample_at(s: vec2<u32>) -> u32 {
  let n = head_word(LIGHT_HEAD_SAMPLES_ACROSS);
  return head_word(LIGHT_HEAD_SAMPLES) + ((s.y % n) * n + s.x % n) * SAMPLE_WORDS;
}

/// List `which`'s count, its entries after it (`light::listed_at`): the corners' before the
/// fed's, the claimed's after.
fn listed_at(which: u32) -> u32 {
  let fed = head_word(LIGHT_HEAD_FED);
  if (which == LIGHT_LISTED_CORNERS) {
    let n = beam_across();
    return fed - 1u - n * n;
  }
  if (which == LIGHT_LISTED_CLAIMED) {
    let n = head_word(LIGHT_HEAD_TILES);
    return fed + 1u + n * n;
  }
  return fed;
}

/// The list what feeds a claimed tile at `level` is listed in (light.rs `listed`).
fn listed(level: u32) -> u32 {
  if (level == LEVEL_CORNER) {
    return LIGHT_LISTED_CORNERS;
  }
  if (level == LEVEL_RAY) {
    return LIGHT_LISTED_CLAIMED;
  }
  return LIGHT_LISTED_FED;
}

/// The slot box texel `p` lies in: `slot_texel` the other way (`Map::slot`).
fn texel_slot(p: vec2<u32>) -> vec2<u32> {
  return vec2<u32>(vec2<i32>(p) + light_corner()) & vec2<u32>(head_word(LIGHT_HEAD_SIZE) - 1u);
}

/// What of `level` the thread at `id` of its pass takes (`Map::take`): itself where the cast
/// begins its epoch, else off the list of what feeds the tiles it claimed, a workgroup of 8 × 8
/// `LISTED_LANES` and an entry `listed_per` of them; none past the list.
fn light_take(level: u32, id: vec2<u32>) -> vec2<u32> {
  if (head_word(LIGHT_HEAD_EPOCH) == head_word(LIGHT_HEAD_CAST)) {
    return id;
  }
  let place = ((id.y / 8u) * LIGHT_LIST_ACROSS + id.x / 8u) * LIGHT_LISTED_LANES + (id.y % 8u) * 8u + id.x % 8u;
  var across = 1u;
  if (level == LEVEL_SAMPLE) {
    across = LIGHT_TILE / LIGHT_SAMPLE;
  } else if (level == LEVEL_RAY) {
    across = LIGHT_TILE;
  }
  let per = across * across;
  let e = place / per;
  let k = place % per;
  let at = listed_at(listed(level));
  if (e >= beams[at]) {
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

/// The tile of the box what `level` at `at` lies on (`Map::on`): a cell's its own number, a
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

/// Whether a tile from `lo_at` to `hi_at` is one the cast casts (light.rs `casts_in`): tile
/// by tile where they are few, else by the squares they lie in (`light::coarse_at`); none past
/// the box.
fn casts_in(lo_at: vec2<i32>, hi_at: vec2<i32>) -> bool {
  let n = i32(head_word(LIGHT_HEAD_TILES));
  let lo = max(lo_at, vec2<i32>(0));
  let hi = min(hi_at, vec2<i32>(n - 1i));
  if (lo.x > hi.x || lo.y > hi.y) {
    return false;
  }
  let few = (hi.x - lo.x + 1i) * (hi.y - lo.y + 1i) <= 16i;
  var k = 1i;
  var first = head_word(LIGHT_HEAD_READS);
  var row = n;
  if (!few) {
    k = i32(LIGHT_COARSE);
    first += u32(n * n);
    row = n / i32(LIGHT_COARSE);
  }
  let number = head_word(LIGHT_HEAD_CAST);
  for (var j = lo.y / k; j <= hi.y / k; j += 1i) {
    for (var i = lo.x / k; i <= hi.x / k; i += 1i) {
      if (beams[first + u32(j * row + i)] == number) {
        return true;
      }
    }
  }
  return false;
}

/// Whether the cast needs what `level` at `at` gives (`Map::needs`): one of the tiles it
/// feeds, the tile it lies on and its reach about it (`Map::feeds`, `Map::reach`).
fn light_needs(level: u32, at: vec2<u32>) -> bool {
  let on = light_on(level, at);
  var lo = vec2<i32>(0);
  var hi = vec2<i32>(0);
  if (level == LEVEL_CELL || level == LEVEL_SAMPLE) {
    lo = vec2<i32>(-1i);
    hi = vec2<i32>(1i);
  } else if (level == LEVEL_CORNER) {
    // The cells whose window, `2 + 2 · BEAM_WINDOW` beam rays a side, holds it.
    let w = i32(LIGHT_BEAM_WINDOW);
    lo = vec2<i32>(-w - 2i);
    hi = vec2<i32>(w + 1i);
  }
  return casts_in(on + lo, on + hi);
}
