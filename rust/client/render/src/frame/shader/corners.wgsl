// pxl-render/corners.wgsl

// The beam rays as the queue's items (queue_walk.wgsl, queue_last.wgsl), an observer's as its
// beam.eye.wgsl or beam.light.wgsl lays them: each a ray from the root, its line kept as it
// goes (walk.wgsl), parked at its state's place (`corner_state_at`; parked.wgsl) with its hand
// set to `beam::HAND_PARKED` while it has not ended, so the cells whose window holds it start
// their rays from the root (cell.wgsl, light_cell.wgsl), and its line ending at the parent of
// the node it was left at, whose handed words it holds, so what reads the line off a cell
// (resume.wgsl, face.wgsl) reads only what was asked; its hand set when it ends.

/// A beam ray by its item word, its column under its row.
fn corner_of(item: u32) -> vec2<u32> {
  return vec2<u32>(item & 0xffffu, item >> 16u);
}

/// The items listed (listed.wgsl).
fn item_count() -> u32 {
  return corners_listed();
}

/// The item at place `k` of the list (listed.wgsl).
fn item_at(k: u32) -> u32 {
  let c = corner_queued(k);
  return c.x | (c.y << 16u);
}

fn item_aim(item: u32) -> Aim {
  return beam_aim(corner_of(item));
}

/// From the root, its line's origin laid where its observer keeps one.
fn item_start(item: u32, aim: Aim) -> Start {
  beam_lay_origin(aim.line, aim.origin);
  return root_start();
}

fn park(item: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) {
  let c = corner_of(item);
  let at = corner_state_at(c);
  beams[at] = bitcast<u32>(t);
  beams[at + 1u] = corner.x;
  beams[at + 2u] = corner.y;
  beams[at + 3u] = corner.z;
  beams[at + 4u] = parked_word(zoom, roots, step);
  beams[beam_hand_at(c)] = HAND_PARKED;
  if (zoom != 0u) {
    let n = beam_line(c) + LINE_NODE;
    beams[n + NODE_ZOOM] = zoom - 1u;
    let up = corner & vec3<u32>(~under(zoom - 1u));
    beams[n + NODE_CORNER] = up.x;
    beams[n + NODE_CORNER + 1u] = up.y;
    beams[n + NODE_CORNER + 2u] = up.z;
  }
}

fn parked(item: u32) -> bool {
  return (beams[corner_state_at(corner_of(item)) + 4u] & PARKED) != 0u;
}

/// Every beam ray of a run afresh in its first walk: none goes on from a cast before, as the
/// cells read every hand this cast writes. The second walk takes those the first parked.
fn taken(item: u32, again: bool) -> u32 {
  if (again) {
    return select(TAKEN_NONE, TAKEN_PARKED, parked(item));
  }
  return TAKEN_FRESH;
}

fn parked_start(item: u32) -> Start {
  let at = corner_state_at(corner_of(item));
  return parked_from(beams[at], vec3<u32>(beams[at + 1u], beams[at + 2u], beams[at + 3u]), beams[at + 4u]);
}

/// How far the rays about it start (`beam_hand`), its state no longer parked.
fn finish(item: u32, aim: Aim, w: Walked) {
  let c = corner_of(item);
  beams[corner_state_at(c) + 4u] = 0u;
  beams[beam_hand_at(c)] = bitcast<u32>(beam_hand(w.t));
}

/// A beam ray the frame's budget does not reach: parked, its hand `HAND_PARKED`, which owes
/// nothing, the rays of its cells walking from the root.
fn unreached(item: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) {
  park(item, t, corner, zoom, roots, step);
}
