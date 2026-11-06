// pxl-render/record_put.wgsl

// A stop into what draws the node cache's entries, the one way every ray's stop goes in
// (pxl-engine record.rs): its node's entry found (record.wgsl `find`), whole where the stop
// found it so, the face it entered set, and that face's word asked the first time it is
// (face.wgsl), the ask laid among the asks it is handed (`Asks`). The samples
// (sample_box.wgsl) and the missed pixels' hits (record_stops.wgsl) each make their stops and
// call `observe_stop`.

/// The node cache, whose entries' states a stop sets (`node_cache::W_STATE`) through atomics,
/// as many stops of one node set them at once.
@group(0) @binding(9) var<storage, read_write> node_cache: array<atomic<u32>>;

fn node_cache_word(k: u32) -> u32 {
  return atomicLoad(&node_cache[k]);
}

/// The raster's args, then the faces' asks' at `RASTER_ASKED_AT` and `RASTER_STOP_ASKED_AT`
/// (pxl-engine `raster::ARGS_INSTANCES`, `raster::ASKED_AT`, `raster::STOP_ASKED_AT`).
@group(0) @binding(16) var<storage, read_write> args: array<atomic<u32>>;

/// What a ray found where it stopped on a node, the node apart (`Kept`; pxl-engine
/// `record`): its answer, whether it lacks, whether the ray stopped on it whole, coarser than
/// it asked there (`record::stopped_whole`, `record::light_whole`), the face its ray entered
/// (2·axis, plus one where the ray runs up the axis), and the first word of the beam ray's
/// line that face's word is asked off (`beam::ASK_LINE`).
struct Stop {
  asset: vec2<u32>,
  lacks: bool,
  whole: bool,
  face: u32,
  line: u32,
}

/// Where a pass lays its faces' asks: its args among the raster's (`RASTER_ASKED_AT` or
/// `RASTER_STOP_ASKED_AT`), the first word of its list among the beam rays' words, and the
/// list's room.
struct Asks {
  args: u32,
  at: u32,
  room: u32,
}

fn args_add(i: u32, v: u32) -> u32 {
  if (i < arrayLength(&args)) {
    return atomicAdd(&args[i], v);
  }
  return 0u;
}

fn args_most(i: u32, v: u32) {
  if (i < arrayLength(&args)) {
    atomicMax(&args[i], v);
  }
}

fn eye_origin() -> vec3<i32> {
  return vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
}

/// The node at word `k` of the beam rays' words (`beam::NODE_WORDS`), as a walk from `origin`
/// holds it, its roots world-absolute.
fn kept_at(k: u32, origin: vec3<i32>) -> Kept {
  let roots = vec3<i32>(i32(beams[k + NODE_ROOTS]), i32(beams[k + NODE_ROOTS + 1u]), i32(beams[k + NODE_ROOTS + 2u]));
  return Kept(
    beams[k + NODE_ZOOM],
    vec3<u32>(beams[k + NODE_CORNER], beams[k + NODE_CORNER + 1u], beams[k + NODE_CORNER + 2u]),
    roots + origin_roots(origin),
  );
}

/// The side stop `p` entered its entry at word `e` through set (`raster::faces`, a bit a side),
/// and, where its observer reads faces' words (`asks`' room, the eye's), set asked
/// (`record::ASKED`): the first time it is, what lies about it told at once where it needs no
/// asking, a lack's none and a cell short of its cube its box's (`face::BOX`), else asked of
/// the faces' pass off the stop's line, laid among `asks`, its node the entry
/// (`record::TO_RECORD`). An observer that reads none (the light's) sets the side alone, a
/// box's word told the first time it is.
fn observe(p: Stop, e: u32, asks: Asks) {
  let bit = 1u << p.face;
  let asked = bit << RECORD_ASKED_SHIFT;
  let reads = asks.room != 0u;
  let mark = select(bit, bit | asked, reads);
  let state = e + W_STATE;
  if ((node_cache_word(state) & mark) == mark) {
    return;
  }
  let was = atomicOr(&node_cache[state], mark);
  let first_asked = reads && (was & asked) == 0u;
  if (p.lacks) {
    return;
  }
  if (((p.asset.x >> FILL_SHIFT) & (EIGHTHS - 1u)) != 0u) {
    if ((was & bit) == 0u || first_asked) {
      atomicStore(&node_cache[e + W_FACES + p.face], FACE_BOX);
    }
    return;
  }
  if (!first_asked) {
    return;
  }
  let place = args_add(asks.args + RASTER_ARGS_CNT, 1u);
  if (place >= asks.room) {
    return;
  }
  args_most(asks.args + RASTER_ARGS_INSTANCES, in_room(place + 1u, asks.room));
  // The faces' pass's workgroups raised to hold it, by the ask that opens one.
  if (place % RASTER_FACES_LANES == 0u) {
    args_most(asks.args + RASTER_ARGS_FACES, workgroups(place + 1u, asks.room, RASTER_FACES_LANES));
  }
  let a = asks.at + place * ASK_WORDS;
  beams[a + ASK_NODE] = e;
  beams[a + ASK_FACE] = p.face;
  beams[a + ASK_LINE] = p.line;
  beams[a + ASK_TO] = RECORD_TO | (e + W_FACES + p.face);
}

/// Stop `p` on the node `k` into what draws it: its entry found (`find`), marked in the frame's
/// number where it draws from now on (`node_cache::W_SEEN`), set whole where the stop found it
/// so (`RECORD_WHOLE`, the world's, which whichever ray stops on it whole first sets), and its
/// face set (`observe`). None where no slot keeps its node: its pixel walks again.
fn observe_stop(k: Kept, p: Stop, asks: Asks) {
  let slot = find(k.zoom, idx_of(k.corner, k.zoom, k.roots, vec3<i32>(0i)), p.asset, p.lacks);
  if (slot == NO_ENTRY) {
    return;
  }
  let e = entry_at(slot);
  let state = node_cache_word(e + W_STATE);
  // Drawable from now: no claim takes it before a draw draws it (`record::drawn`).
  if ((state & (RECORD_FACES | RECORD_ASKED)) == 0u) {
    atomicStore(&node_cache[e + W_SEEN], record_frame());
  }
  if (p.whole && (state & RECORD_WHOLE) == 0u) {
    atomicOr(&node_cache[e + W_STATE], RECORD_WHOLE);
  }
  observe(p, e, asks);
}
