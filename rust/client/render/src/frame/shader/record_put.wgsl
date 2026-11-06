// pxl-render/record_put.wgsl

// A stop into the eye's record of voxels, the one way every ray's stop goes in (pxl-engine
// record.rs): its key's entry found, or claimed and written, one key one entry; marked seen;
// the face it entered set, and that face's word asked the first time it is (face.wgsl), the
// ask laid among the asks it is handed (`Asks`). The samples (sample_box.wgsl) and the missed
// pixels' hits (record_stops.wgsl) each make their stops and call `upsert`.

/// The raster's args, then the faces' asks' at `RASTER_ASKED_AT` and `RASTER_STOP_ASKED_AT`,
/// and the hits put off (pxl-engine `raster::ARGS_INSTANCES`, `raster::ASKED_AT`,
/// `raster::STOP_ASKED_AT`, `raster::DEFERRED_AT`).
@group(0) @binding(16) var<storage, read_write> args: array<atomic<u32>>;

/// None.
const NO_ENTRY: u32 = 0xffffffffu;
/// What a stop that met its key's claim takes: none yet, put off past the dispatch's end
/// (pxl-engine `record`).
const MET_CLAIM: u32 = 0xfffffffeu;
/// The times a stop reads its bucket again after another took the slot it would.
const CLAIM_TRIES: u32 = 4u;

/// What a ray found where it stopped on a node, the node's key apart (`Kept`; pxl-engine
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
/// holds it, at band `band`, as the record keys it (`record::Key::of`).
fn kept_at(k: u32, band: u32, origin: vec3<i32>) -> Kept {
  let roots = vec3<i32>(i32(beams[k + NODE_ROOTS]), i32(beams[k + NODE_ROOTS + 1u]), i32(beams[k + NODE_ROOTS + 2u]));
  return Kept(
    beams[k + NODE_ZOOM],
    vec3<u32>(beams[k + NODE_CORNER], beams[k + NODE_CORNER + 1u], beams[k + NODE_CORNER + 2u]),
    roots + origin_roots(origin),
    band,
  );
}

/// The slot of the record that holds `k`, `NO_ENTRY` where none does: the first of its bucket
/// that holds it.
fn find(k: Kept) -> u32 {
  let b = kept_bucket(k);
  for (var i = 0u; i < RECORD_BUCKET; i += 1u) {
    let e = record_at(b + i);
    let held = record_word(e + R_HELD);
    if ((held & RECORD_HELD) != 0u && entry_holds(e, held, k)) {
      return b + i;
    }
  }
  return NO_ENTRY;
}

/// Whether a stop of `k` that claimed slot `pick` of the bucket from `b` lets its claim go:
/// another slot holds `k`, or `k`'s claim stands before it in the bucket. Of the claims of
/// one key that meet, the first stands: a claim yields only to a held entry, which stands,
/// or to a claim before it, and the first only to a held one.
fn yields(k: Kept, b: u32, pick: u32) -> bool {
  let mine = kept_claim(k);
  for (var i = 0u; i < RECORD_BUCKET; i += 1u) {
    if (b + i == pick) {
      continue;
    }
    let e = record_at(b + i);
    let held = record_word(e + R_HELD);
    if ((held & RECORD_HELD) != 0u && entry_holds(e, held, k)) {
      return true;
    }
    if (held == mine && b + i < pick) {
      return true;
    }
  }
  return false;
}

/// Stop `p` of `k` written into slot `slot` it claimed: its key, its answer, its state, seen
/// this frame, no face drawn yet; its held word last.
fn write_entry(k: Kept, p: Stop, slot: u32) {
  let e = record_at(slot);
  atomicStore(&record[e + R_CORNER], k.corner.x);
  atomicStore(&record[e + R_CORNER + 1u], k.corner.y);
  atomicStore(&record[e + R_CORNER + 2u], k.corner.z);
  atomicStore(&record[e + R_ROOTS], u32(k.roots.x));
  atomicStore(&record[e + R_ROOTS + 1u], u32(k.roots.y));
  atomicStore(&record[e + R_ROOTS + 2u], u32(k.roots.z));
  atomicStore(&record[e + R_ASSET], p.asset.x);
  atomicStore(&record[e + R_ASSET + 1u], p.asset.y);
  let lacks = select(0u, RECORD_LACKS, p.lacks);
  let whole = select(0u, RECORD_WHOLE, p.whole);
  atomicStore(&record[e + R_STATE], lacks | whole);
  atomicStore(&record[e + R_SEEN], record_frame());
  for (var face = 0u; face < 6u; face += 1u) {
    atomicStore(&record[e + R_FACES + face], 0u);
  }
  atomicStore(&record[e + R_HELD], kept_held(k));
}

/// Stop `p` of `k`'s slot: the one that holds its key; `MET_CLAIM` where its key's claim holds
/// one, which it waits on no lane for; else one it claims and writes, the first free slot of
/// its bucket, else the one least lately seen of those no pixel took this frame or the last
/// (`record::victim`). `NO_ENTRY` where every slot was, or where it yielded to a claim not
/// yet held.
fn claim(k: Kept, p: Stop) -> u32 {
  let b = kept_bucket(k);
  let mine = kept_claim(k);
  let frame = record_frame();
  for (var tries = 0u; tries < CLAIM_TRIES; tries += 1u) {
    var pick = NO_ENTRY;
    var was = 0u;
    var oldest = 0xffffffffu;
    var free = false;
    var met = false;
    for (var i = 0u; i < RECORD_BUCKET; i += 1u) {
      let e = record_at(b + i);
      let held = record_word(e + R_HELD);
      if (held == 0u) {
        if (!free) {
          free = true;
          pick = b + i;
          was = 0u;
        }
        continue;
      }
      if (held == mine) {
        met = true;
        break;
      }
      if ((held & RECORD_HELD) == 0u) {
        continue;
      }
      if (entry_holds(e, held, k)) {
        return b + i;
      }
      let seen = record_word(e + R_SEEN);
      if (!free && seen + 2u <= frame && seen < oldest) {
        oldest = seen;
        pick = b + i;
        was = held;
      }
    }
    if (met) {
      return MET_CLAIM;
    }
    if (pick == NO_ENTRY) {
      return NO_ENTRY;
    }
    let r = atomicCompareExchangeWeak(&record[record_at(pick) + R_HELD], was, mine);
    if (r.exchanged) {
      // Read the bucket again: a stop of the same key that claimed another slot meanwhile
      // keeps the first; this one lets its slot go free.
      if (yields(k, b, pick)) {
        atomicStore(&record[record_at(pick) + R_HELD], 0u);
        return find(k);
      }
      write_entry(k, p, pick);
      return pick;
    }
  }
  return NO_ENTRY;
}

/// The side stop `p` entered its entry at word `e` through set (`raster::faces`, a bit a side),
/// and, the first time it is, what lies about it told at once where it needs no asking, a lack's none and a
/// cell short of its cube its box's (`face::BOX`), else asked of the faces' pass off the stop's
/// line, laid among `asks`, its node the entry (`record::TO_RECORD`).
fn observe(p: Stop, e: u32, asks: Asks) {
  let bit = 1u << p.face;
  let state = e + R_STATE;
  if ((record_word(state) & bit) != 0u || (atomicOr(&record[state], bit) & bit) != 0u) {
    return;
  }
  if (p.lacks) {
    return;
  }
  if (((p.asset.x >> FILL_SHIFT) & (EIGHTHS - 1u)) != 0u) {
    atomicStore(&record[e + R_FACES + p.face], FACE_BOX);
    return;
  }
  // An observer that reads no face's word lays its asks in no room (the light's).
  if (asks.room == 0u) {
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
  beams[a + ASK_TO] = RECORD_TO | (e + R_FACES + p.face);
}

/// Stop `p` of `k` into the record: its key's entry found, or, where `claims`, claimed and
/// written; marked seen and its face set (`observe`). Its slot; `NO_ENTRY` where it took
/// none; `MET_CLAIM` where it met its key's claim, for its caller to put it off past the
/// dispatch's end.
fn upsert(k: Kept, p: Stop, claims: bool, asks: Asks) -> u32 {
  var slot = find(k);
  if (slot == NO_ENTRY && claims) {
    slot = claim(k, p);
  }
  if (slot == NO_ENTRY || slot == MET_CLAIM) {
    return slot;
  }
  let e = record_at(slot);
  let frame = record_frame();
  if (record_word(e + R_SEEN) != frame) {
    atomicStore(&record[e + R_SEEN], frame);
  }
  observe(p, e, asks);
  return slot;
}
