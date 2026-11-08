// pxl-render/record_put.wgsl

// Writes a ray's stop into the record (pxl-engine record.rs, the third of its writers). Every
// stop goes in this way: the samples' (sample_box.wgsl) and the missed pixels' hits
// (record_hits.wgsl) each call `observe_hit`. A stop finds its node's entry (record.wgsl
// `find`), marks the entry whole where it stopped on it whole, and sets the face its ray
// entered. The first stop to enter a face asks the faces' pass (face.wgsl) for that face's
// word, with an ask laid among the asks it is handed (`AskList`).

/// The node cache. A stop sets its entry's state (`node_cache::STATES_AT`) with atomics,
/// because many stops on one node set it at once.
@group(0) @binding(9) var<storage, read_write> node_cache: array<atomic<u32>>;

fn node_cache_word(k: u32) -> u32 {
  return atomicLoad(&node_cache[k]);
}

/// The raster's arguments, followed by the arguments of the two sets of faces' asks at
/// `RASTER_ASKED_AT` and `RASTER_HIT_ASKED_AT`, each laid out as the draw's are (pxl-engine
/// `raster::ARGS_INSTANCES`, `raster::ASKED_AT`, `raster::HIT_ASKED_AT`).
@group(0) @binding(16) var<storage, read_write> args: array<atomic<u32>>;

/// What a ray found where it stopped on a node (pxl-engine `record`); the node itself is
/// passed apart, as a `Node`. `asset` is the node's answer, and `lacks` says the node's data is
/// not held. `whole` says the node is coarser than the zoom the ray asked there
/// (`record::stopped_whole`, `record::light_whole`). `face` is the face the ray entered: twice
/// the axis, plus one where the ray runs up the axis. `line` is the first word of the beam
/// ray's line from which that face's word is asked (`beam::ASK_LINE`).
struct Hit {
  asset: vec2<u32>,
  lacks: bool,
  whole: bool,
  face: u32,
  line: u32,
}

/// Where a pass lays the faces' asks it opens. `args` is the first word of their arguments
/// among the raster's (`RASTER_ASKED_AT` or `RASTER_HIT_ASKED_AT`), `at` the first word of
/// their list among the beam rays' words, and `cap` the most asks the list holds. A `cap` of 0
/// means the observer reads no face's word.
struct AskList {
  args: u32,
  at: u32,
  cap: u32,
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

/// The node laid at word `k` of the beam rays' words (pxl-engine `beam::NODE_WORDS`), as a
/// walk from `origin` holds it, with its roots counted over the whole world (`Node`).
fn node_at(k: u32, origin: vec3<i32>) -> Node {
  let roots = vec3<i32>(i32(beams[k + NODE_ROOTS]), i32(beams[k + NODE_ROOTS + 1u]), i32(beams[k + NODE_ROOTS + 2u]));
  return Node(
    beams[k + NODE_ZOOM],
    vec3<u32>(beams[k + NODE_CORNER], beams[k + NODE_CORNER + 1u], beams[k + NODE_CORNER + 2u]),
    roots + origin_roots(origin),
  );
}

/// Sets, in the entry of slot `slot`, the bit of the side that stop `p` entered it through
/// (pxl-engine `raster::faces`). An observer that reads faces' words, which is the eye, also
/// sets the side's asked bit (`record::ASKED`). The first time a side is asked, its word is
/// settled. A lack's side takes no word. A cell that fills only part of its cube is a box apart
/// from everything around it, so its side takes `FACE_BOX` at once (pxl-engine `face::BOX`).
/// Any other side is asked of the faces' pass from the stop's line, by an ask laid among
/// `asks` whose target is the entry's word (`record::TO_RECORD`). An observer that reads no
/// words, which is the light, sets only the side, and writes a box's word the first time it
/// sets the side.
fn observe(p: Hit, slot: u32, asks: AskList) {
  let bit = 1u << p.face;
  let asked = bit << RECORD_ASKED_SHIFT;
  let reads = asks.cap != 0u;
  let mark = select(bit, bit | asked, reads);
  let state = state_at(slot);
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
      atomicStore(&node_cache[face_at(slot, p.face)], FACE_BOX);
    }
    return;
  }
  if (!first_asked) {
    return;
  }
  let place = args_add(asks.args + RASTER_ARGS_CNT, 1u);
  if (place >= asks.cap) {
    return;
  }
  args_most(asks.args + RASTER_ARGS_INSTANCES, in_cap(place + 1u, asks.cap));
  // The ask that opens a workgroup of the faces' pass raises the pass's workgroup count.
  if (place % RASTER_FACES_LANES == 0u) {
    args_most(asks.args + RASTER_ARGS_FACES, workgroups(place + 1u, asks.cap, RASTER_FACES_LANES));
  }
  let a = asks.at + place * ASK_WORDS;
  beams[a + ASK_NODE] = entry_at(slot);
  beams[a + ASK_FACE] = p.face;
  beams[a + ASK_LINE] = p.line;
  beams[a + ASK_TO] = RECORD_TO | face_at(slot, p.face);
}

/// Writes stop `p` on the node `k` into the entry that draws the node (`find`). Where no slot
/// keeps the node, nothing is written, and the stop's pixel walks again. Where the stop is
/// whole, the entry is set whole (`RECORD_WHOLE`). That is a fact of the world, so the first
/// ray of either observer to stop on the node whole sets it. Then the stop's face is set
/// (`observe`).
fn observe_hit(k: Node, p: Hit, asks: AskList) {
  let slot = find(k.zoom, idx_of(k.corner, k.zoom, k.roots, vec3<i32>(0i)), p.asset, p.lacks);
  if (slot == NO_ENTRY) {
    return;
  }
  let state = node_cache_word(state_at(slot));
  // An entry with no face set becomes drawable with this stop. Marking it with this frame's
  // number keeps eviction from taking it before a draw draws it (pxl-engine `record::drawn`).
  if ((state & (RECORD_FACES | RECORD_ASKED)) == 0u) {
    atomicStore(&node_cache[drawn_at(slot)], record_frame());
  }
  if (p.whole && (state & RECORD_WHOLE) == 0u) {
    atomicOr(&node_cache[state_at(slot)], RECORD_WHOLE);
  }
  observe(p, slot, asks);
}
