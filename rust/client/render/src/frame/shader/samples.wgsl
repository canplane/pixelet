// pxl-render/samples.wgsl

// The samples as the queue's items (queue_walk.wgsl, queue_last.wgsl), an observer's as its
// sample.eye.wgsl or sample.light.wgsl lays them: each a ray off its cell's start, parked in its
// own words while it has not ended (pxl-engine `beam::SAMPLE_PARKED`), its hit put there once
// it ends (sample_put.wgsl).

/// A sample by its item word, its column under its row.
fn sample_of(item: u32) -> vec2<u32> {
  return vec2<u32>(item & 0xffffu, item >> 16u);
}

/// The items listed (listed.wgsl).
fn item_count() -> u32 {
  return samples_listed();
}

/// The item at place `k` of the list (listed.wgsl).
fn item_at(k: u32) -> u32 {
  let s = sample_queued(k);
  return s.x | (s.y << 16u);
}

fn item_aim(item: u32) -> Aim {
  return sample_aim(sample_of(item));
}

fn item_start(item: u32, aim: Aim) -> Start {
  return sample_start(aim, sample_of(item));
}

/// A sample's ray parked at its node `corner`, `zoom`, `roots`, `t` along it, `step` steps
/// taken, in its own words (`beam::SAMPLE_PARKED_T`, `beam::SAMPLE_PARKED_STATE`; parked.wgsl).
fn park(item: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) {
  let at = sample_word(sample_of(item));
  beams[at + SAMPLE_NODE + NODE_ZOOM] = SAMPLE_PARKED;
  beams[at + SAMPLE_PARKED_T] = bitcast<u32>(t);
  beams[at + SAMPLE_NODE + NODE_CORNER] = corner.x;
  beams[at + SAMPLE_NODE + NODE_CORNER + 1u] = corner.y;
  beams[at + SAMPLE_NODE + NODE_CORNER + 2u] = corner.z;
  beams[at + SAMPLE_PARKED_STATE] = parked_word(zoom, roots, step);
}

fn parked(item: u32) -> bool {
  return beams[sample_word(sample_of(item)) + SAMPLE_NODE + NODE_ZOOM] == SAMPLE_PARKED;
}

/// What a sample's first walk of a run takes: where this cast's samples go on from the cast
/// before's (`samples_resume`), the state it is parked in, an ended one none; else every sample
/// afresh. The second walk takes those the first parked.
fn taken(item: u32, again: bool) -> u32 {
  if (again || samples_resume()) {
    return select(TAKEN_NONE, TAKEN_PARKED, parked(item));
  }
  return TAKEN_FRESH;
}

fn parked_start(item: u32) -> Start {
  let at = sample_word(sample_of(item));
  let corner = vec3<u32>(
    beams[at + SAMPLE_NODE + NODE_CORNER],
    beams[at + SAMPLE_NODE + NODE_CORNER + 1u],
    beams[at + SAMPLE_NODE + NODE_CORNER + 2u],
  );
  return parked_from(beams[at + SAMPLE_PARKED_T], corner, beams[at + SAMPLE_PARKED_STATE]);
}

fn finish(item: u32, aim: Aim, w: Walked) {
  sample_put(sample_word(sample_of(item)), w);
}

/// A sample the frame's budget does not reach: parked, and owed as its observer owes it
/// (`sample_owed`).
fn unreached(item: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) {
  park(item, t, corner, zoom, roots, step);
  sample_owed();
}
