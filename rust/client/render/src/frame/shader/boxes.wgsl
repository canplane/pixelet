// pxl-render/boxes.wgsl

// The voxels the light's samples stopped on written as boxes for the raster's draw
// (raster.wgsl), over the samples' words (resume.wgsl `beams`) and their observer
// (sample.light.wgsl) (pxl-engine raster.rs); the eye's are its record's (sample_box.wgsl,
// record_draw.wgsl). The boxes, the cast's table of voxels past them, the voxels' words, the
// raster's args, and the asks of what lies about each voxel's faces (face.wgsl). Two passes over the samples: the first
// (`claim_box`) has each sample that heads its node's run, no sample before it across or up
// stopped on the node, claim the node's slot and the one that claims it write the voxel; the
// second (`observe_box`) has every sample find its node's voxel, or write one of its own
// where none is, and draw the face it entered across.

/// The boxes, then the cast's table of voxels (pxl-engine `raster::keys_at`), cleared before
/// the cast writes them.
@group(0) @binding(14) var<storage, read_write> boxes: array<atomic<u32>>;
/// The raster's args, then the faces' asks' at `RASTER_ASKED_AT` (pxl-engine
/// `raster::ARGS_INSTANCES`, `raster::ASKED_AT`).
@group(0) @binding(16) var<storage, read_write> args: array<atomic<u32>>;

/// No box written, no slot.
const NO_BOX: u32 = 0xffffffffu;

fn box_put(i: u32, v: u32) {
  if (i < arrayLength(&boxes)) {
    atomicStore(&boxes[i], v);
  }
}

/// `v` added to word `i` of the args: what it held, 0 past them.
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

fn args_at(i: u32) -> u32 {
  if (i < arrayLength(&args)) {
    return atomicLoad(&args[i]);
  }
  return 0u;
}

/// Whether samples `a` and `b` stopped on one voxel: one node at one band (pxl-engine
/// `raster::key`).
fn same_key(a: vec2<u32>, b: vec2<u32>) -> bool {
  let ka = sample_word(a) + SAMPLE_NODE;
  let kb = sample_word(b) + SAMPLE_NODE;
  for (var i = 0u; i < NODE_WORDS; i += 1u) {
    if (beams[ka + i] != beams[kb + i]) {
      return false;
    }
  }
  return sample_band(a) == sample_band(b);
}

fn stopped(s: vec2<u32>) -> bool {
  return beams[sample_word(s) + SAMPLE_NODE + NODE_ZOOM] != SAMPLE_NONE;
}

/// The sample a number names (`s.y · across + s.x`).
fn sample_numbered(number: u32) -> vec2<u32> {
  let n = sample_lattice().x;
  return vec2<u32>(number % n, number / n);
}

/// Where sample `s`'s voxel first looks in the table of `keys` slots.
fn key_slot(s: vec2<u32>, keys: u32) -> u32 {
  let k = sample_word(s) + SAMPLE_NODE;
  var h = sample_band(s) * 0x9e3779b9u + 0x7f4a7c15u;
  for (var i = 0u; i < NODE_WORDS; i += 1u) {
    h = (h ^ beams[k + i]) * 0x85ebca6bu;
    h ^= h >> 15u;
  }
  return h % keys;
}

/// The table of voxels: its first word and its slots, `RASTER_KEYS` a sample.
fn keys_at() -> u32 {
  let n = sample_lattice();
  return n.x * n.y * RASTER_BOX_WORDS;
}

fn keys() -> u32 {
  let n = sample_lattice();
  return n.x * n.y * RASTER_KEYS;
}

/// The slot sample `s`, numbered `number`, claims for its voxel; `NO_BOX` where another
/// sample of its voxel holds it, or none is free (pxl-engine `raster::KEYS`).
fn claim(s: vec2<u32>, number: u32) -> u32 {
  let at = keys_at();
  let room = keys();
  var slot = key_slot(s, room);
  for (var tries = 0u; tries < 2u * room; tries += 1u) {
    let w = at + slot * RASTER_KEY_WORDS;
    let r = atomicCompareExchangeWeak(&boxes[w], 0u, number + 1u);
    if (r.exchanged) {
      return slot;
    }
    // A weak exchange may fail with the slot free: tried again.
    if (r.old_value != 0u) {
      if (same_key(s, sample_numbered(r.old_value - 1u))) {
        return NO_BOX;
      }
      slot = (slot + 1u) % room;
    }
  }
  return NO_BOX;
}

/// The voxel of sample `s` the table holds: its number, `NO_BOX` where none does.
fn find(s: vec2<u32>) -> u32 {
  let at = keys_at();
  let room = keys();
  var slot = key_slot(s, room);
  for (var tries = 0u; tries < room; tries += 1u) {
    let w = at + slot * RASTER_KEY_WORDS;
    let held = atomicLoad(&boxes[w]);
    if (held == 0u) {
      return NO_BOX;
    }
    if (same_key(s, sample_numbered(held - 1u))) {
      return atomicLoad(&boxes[w + 1u]) - 1u;
    }
    slot = (slot + 1u) % room;
  }
  return NO_BOX;
}

/// A box in units from the eye.
struct Box {
  lo: vec3<f32>,
  hi: vec3<f32>,
}

/// The node at word `k` as drawn from the origin `o`, the cell its word `asset` keeps
/// inset as it fills (pxl-engine cast.rs `asset_box`, `bounds`, `fill_bounds`).
fn asset_box(o: vec3<i32>, k: u32, asset: u32) -> Box {
  let og = vec3<u32>(u32(o.x), u32(o.y) + Y_START, u32(o.z) + Z_START);
  let corner = vec3<u32>(beams[k + NODE_CORNER], beams[k + NODE_CORNER + 1u], beams[k + NODE_CORNER + 2u]);
  let roots = vec3<i32>(
    i32(beams[k + NODE_ROOTS]),
    i32(beams[k + NODE_ROOTS + 1u]),
    i32(beams[k + NODE_ROOTS + 2u]),
  );
  let lo = units(corner, og, roots);
  let hi = far_of(corner, under(beams[k + NODE_ZOOM]), og, roots);
  let eighths = (asset >> FILL_SHIFT) & (EIGHTHS - 1u);
  if (eighths == 0u) {
    return Box(lo, hi);
  }
  let inset = (hi - lo) * (0.5 * (1.0 - side(eighths)));
  return Box(lo + inset, hi - inset);
}

/// Whether the box lies so near the eye that the raster's near plane may cut it, `tan`
/// the screen's tangents (pxl-engine raster.rs `is_eyed`).
fn is_eyed(b: Box, tan_xy: vec2<f32>) -> bool {
  let v = select(select(vec3<f32>(0.0), b.hi, b.hi < vec3<f32>(0.0)), b.lo, b.lo > vec3<f32>(0.0));
  let k = 2.0 * RASTER_NEAR;
  return v.x * v.x + v.y * v.y + v.z * v.z < k * k * (1.0 + tan_xy.x * tan_xy.x + tan_xy.y * tan_xy.y);
}

/// Sample `s`'s voxel written: its box from `origin`, appended to the boxes and counted in the
/// args, none past the room of the boxes, its number, no face drawn yet and `RASTER_EYED`
/// where it lies about the eye at the screen's tangents `tan` where `eyed` asks; and its words
/// (`beam::VOXEL_WORDS`) at the same place among the voxels. Its number, `NO_BOX` where it
/// wrote none (pxl-engine `raster`, `cast::asset_box`).
fn write_voxel(s: vec2<u32>, origin: vec3<i32>, eyed: bool, tan_xy: vec2<f32>) -> u32 {
  let n = sample_lattice();
  let room = n.x * n.y;
  let k = sample_word(s);
  let b = asset_box(origin, k + SAMPLE_NODE, beams[k + SAMPLE_ASSET]);
  let lacks = beams[k + SAMPLE_LACKS] != 0u;
  let near = eyed && is_eyed(b, tan_xy);
  let box_at = args_add(RASTER_ARGS_CNT, 1u);
  if (box_at >= room) {
    return NO_BOX;
  }
  args_most(RASTER_ARGS_INSTANCES, in_room(box_at + 1u, room));
  let at = box_at * RASTER_BOX_WORDS;
  box_put(at + RASTER_BOX_LO, bitcast<u32>(b.lo.x));
  box_put(at + RASTER_BOX_LO + 1u, bitcast<u32>(b.lo.y));
  box_put(at + RASTER_BOX_LO + 2u, bitcast<u32>(b.lo.z));
  box_put(at + RASTER_BOX_HI, bitcast<u32>(b.hi.x));
  box_put(at + RASTER_BOX_HI + 1u, bitcast<u32>(b.hi.y));
  box_put(at + RASTER_BOX_HI + 2u, bitcast<u32>(b.hi.z));
  box_put(at + RASTER_BOX_ID, (box_at + 1u) | select(0u, RASTER_LACK, lacks));
  box_put(at + RASTER_BOX_FACES, select(0u, RASTER_EYED, near));
  let r = voxels_at() + box_at * VOXEL_WORDS;
  for (var i = 0u; i < NODE_WORDS; i += 1u) {
    beams[r + VOXEL_NODE + i] = beams[k + SAMPLE_NODE + i];
  }
  beams[r + VOXEL_ASSET] = beams[k + SAMPLE_ASSET];
  beams[r + VOXEL_ASSET + 1u] = beams[k + SAMPLE_ASSET + 1u];
  beams[r + VOXEL_LACKS] = beams[k + SAMPLE_LACKS];
  beams[r + VOXEL_BAND] = sample_band(s);
  for (var axis = 0u; axis < 3u; axis += 1u) {
    beams[r + VOXEL_FACES + axis] = 0u;
  }
  return box_at;
}

/// The face sample `s` entered voxel `v` across drawn (`raster::faces`), and, the first time
/// it is, what lies about it told at once where it needs no asking, a lack's none and a cell
/// short of its cube its box's (`face::BOX`), else asked of the faces' pass where the
/// observer reads it, off the line the sample's cell's rays resumed from, left at its axis
/// among the voxel's words (pxl-engine `beam::ASK_WORDS`).
fn observe(s: vec2<u32>, v: u32) {
  let k = sample_word(s);
  let axis = beams[k + SAMPLE_AXIS];
  let bit = 1u << axis;
  let w = v * RASTER_BOX_WORDS + RASTER_BOX_FACES;
  if ((atomicLoad(&boxes[w]) & bit) != 0u || (atomicOr(&boxes[w], bit) & bit) != 0u) {
    return;
  }
  let r = voxels_at() + v * VOXEL_WORDS;
  let fill = (beams[r + VOXEL_ASSET] >> FILL_SHIFT) & (EIGHTHS - 1u);
  if (beams[r + VOXEL_LACKS] != 0u) {
    return;
  }
  if (fill != 0u) {
    beams[r + VOXEL_FACES + axis] = FACE_BOX;
    return;
  }
  if (!SAMPLE_FACE_WORDS) {
    return;
  }
  let n = sample_lattice();
  let room = n.x * n.y;
  let place = args_add(RASTER_ASKED_AT + RASTER_ARGS_CNT, 1u);
  if (place >= room) {
    return;
  }
  args_most(RASTER_ASKED_AT + RASTER_ARGS_INSTANCES, in_room(place + 1u, room));
  // The faces' pass's workgroups raised to hold it, by the ask that opens one.
  if (place % RASTER_FACES_LANES == 0u) {
    args_most(RASTER_ASKED_AT + RASTER_ARGS_FACES, workgroups(place + 1u, room, RASTER_FACES_LANES));
  }
  // The face its ray enters across its axis: 2·axis, plus one where the ray runs up
  // it (miss_word.wgsl `face_of`).
  let dir = sample_aim(s).dir;
  let along = select(select(dir.z, dir.y, axis == 1u), dir.x, axis == 0u);
  let a = args_at(RASTER_ASKED_AT + RASTER_ARGS_ASKS) + place * ASK_WORDS;
  beams[a + ASK_NODE] = r + VOXEL_NODE;
  beams[a + ASK_FACE] = 2u * axis + select(0u, 1u, along > 0.0);
  beams[a + ASK_LINE] = sample_line(s);
  beams[a + ASK_TO] = r + VOXEL_FACES + axis;
}

/// The sample the thread at `id` takes, where the lattice holds it and it stopped.
fn stopped_item(id: vec2<u32>) -> vec2<u32> {
  let n = sample_lattice();
  let s = sample_item(id);
  if (s.x >= n.x || s.y >= n.y || !stopped(s)) {
    return vec2<u32>(NO_BOX);
  }
  return s;
}

/// The first pass: the sample the thread at `id` takes, where it heads its voxel's run, no
/// sample before it across or up stopped on its voxel, claims the voxel's slot, and writes
/// the voxel from `origin` where it claims it (`write_voxel`), its number left in the slot.
fn claim_box(id: vec2<u32>, origin: vec3<i32>, eyed: bool, tan_xy: vec2<f32>) {
  let s = stopped_item(id);
  if (s.x == NO_BOX) {
    return;
  }
  if ((s.x > 0u && same_key(s, s - vec2<u32>(1u, 0u))) || (s.y > 0u && same_key(s, s - vec2<u32>(0u, 1u)))) {
    return;
  }
  let n = sample_lattice();
  let slot = claim(s, s.y * n.x + s.x);
  if (slot == NO_BOX) {
    return;
  }
  let v = write_voxel(s, origin, eyed, tan_xy);
  box_put(keys_at() + slot * RASTER_KEY_WORDS + 1u, v + 1u);
}

/// The second pass: the sample the thread at `id` takes finds its voxel, or writes one of its
/// own where the table holds none, a voxel drawn twice then but never none, and draws the face
/// it entered across (`observe`).
fn observe_box(id: vec2<u32>, origin: vec3<i32>, eyed: bool, tan_xy: vec2<f32>) {
  let s = stopped_item(id);
  if (s.x == NO_BOX) {
    return;
  }
  var v = find(s);
  if (v == NO_BOX) {
    v = write_voxel(s, origin, eyed, tan_xy);
  }
  if (v != NO_BOX) {
    observe(s, v);
  }
}
