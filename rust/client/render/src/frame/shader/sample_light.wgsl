// pxl-render/sample_light.wgsl

// The shadow map as the samples' walks take it (pxl-shader light.rs `Map`): a sample every
// `LIGHT_SAMPLE` slots, taken off what the cast casts, each ray at its tile's band.

const WALK_PASS: u32 = LIGHT_SAMPLES_PASS;
const VISITS_TALLY: u32 = LIGHT_SAMPLE_TALLY;
const ASKED_TALLY: u32 = LIGHT_ASKED_TALLY;

fn sample_lattice() -> vec2<u32> {
  return vec2<u32>(head_word(LIGHT_HEAD_SAMPLES_ACROSS));
}

/// The sample the thread at `id` walks (`Map::take`): its own where the cast begins its epoch,
/// else one of the tiles listed as fed, `NONE` past them.
fn sample_taken(id: vec2<u32>) -> vec2<u32> {
  if (head_word(LIGHT_HEAD_EPOCH) == head_word(LIGHT_HEAD_CAST)) {
    return id;
  }
  let place = ((id.y / SAMPLE_SIDE) * LIGHT_LIST_ACROSS + id.x / SAMPLE_SIDE) * LIGHT_LISTED_LANES
    + (id.y % SAMPLE_SIDE) * SAMPLE_SIDE + id.x % SAMPLE_SIDE;
  let side = LIGHT_TILE / LIGHT_SAMPLE;
  let e = place / (side * side);
  let k = place % (side * side);
  let at = head_word(LIGHT_HEAD_FED);
  if (e >= beams[at]) {
    return vec2<u32>(NONE);
  }
  let t = beams[at + 1u + e];
  let slot = (vec2<u32>(t & 0xffffu, t >> 16u) * LIGHT_TILE + vec2<u32>(light_corner())) & vec2<u32>(head_word(LIGHT_HEAD_SIZE) - 1u);
  return slot / LIGHT_SAMPLE + vec2<u32>(k % side, k / side);
}

/// Whether a tile it may feed, its own and those about it (`Map::reach`), is one this cast
/// casts (`light::reads_at`).
fn sample_needed(s: vec2<u32>) -> bool {
  let on = vec2<i32>(slot_texel(s * LIGHT_SAMPLE) / LIGHT_TILE);
  let tiles = i32(head_word(LIGHT_HEAD_TILES));
  let lo = max(on - vec2<i32>(1), vec2<i32>(0));
  let hi = min(on + vec2<i32>(1), vec2<i32>(tiles - 1));
  let reads = head_word(LIGHT_HEAD_READS);
  let number = head_word(LIGHT_HEAD_CAST);
  for (var j = lo.y; j <= hi.y; j += 1i) {
    for (var i = lo.x; i <= hi.x; i += 1i) {
      if (beams[reads + u32(j * tiles + i)] == number) {
        return true;
      }
    }
  }
  return false;
}

fn sample_word(s: vec2<u32>) -> u32 {
  return head_word(LIGHT_HEAD_SAMPLES) + (s.y * head_word(LIGHT_HEAD_SAMPLES_ACROSS) + s.x) * SAMPLE_WORDS;
}

fn left_at() -> u32 {
  return head_word(LIGHT_HEAD_LEFTS);
}

fn sample_aim(s: vec2<u32>) -> Aim {
  let texel = slot_texel(s * LIGHT_SAMPLE);
  return Aim(
    texel_start(light_corner() + vec2<i32>(texel)),
    light_v3(LIGHT_DIR),
    eye_f32(LIGHT + LIGHT_T_MAX),
    vec2<u32>(texel_band(texel)),
    mix_hash(head_word(LIGHT_HEAD_CAST), s.x | (s.y << 16u)),
  );
}

/// Off the cell of the box its texel lies in (`Map::cell_of`).
fn sample_start(aim: Aim, s: vec2<u32>) -> Start {
  let cell = slot_texel(s * LIGHT_SAMPLE) / LIGHT_BEAM;
  return resumed(
    aim,
    head_word(LIGHT_HEAD_CELLS) + (cell.y * head_word(LIGHT_HEAD_CELLS_ACROSS) + cell.x) * CELL_WORDS,
    LIGHT_LINE_WORDS,
    LIGHT_LINE_ORIGIN,
  );
}
