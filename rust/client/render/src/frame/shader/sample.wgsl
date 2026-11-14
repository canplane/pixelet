// pxl-render/frame/shader/sample.wgsl

// An observer's samples as the queue's items (queue_walk.wgsl, queue_last.wgsl), laid out as
// sample.eye.wgsl or sample.light.wgsl lays them. Each item is a sample's ray, which starts at
// its cell's start. Between its run's walks the ray is parked in the sample's own words
// (pxl-engine `beam::SAMPLE_PARKED`), and once it ends its hit is written there
// (sample_put.wgsl).

fn sample_of(item: u32) -> vec2<u32> {
  return vec2<u32>(item & 0xffffu, item >> 16u);
}

// item_list.wgsl takes the listed items through `item_cnt` and `item_at`.
fn item_cnt() -> u32 {
  return sample_cnt();
}

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

/// Parks a sample's ray in the sample's own words, at the node `corner`, `zoom`, `roots`, at
/// `t` along the ray, after `step` steps (pxl-engine `beam::SAMPLE_PARKED_T`,
/// `beam::SAMPLE_PARKED_STATE`; parked.wgsl). It returns false where the parked items' list
/// has no room (item_list.wgsl `list_parked`).
fn park(item: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) -> bool {
  let at = sample_word(sample_of(item));
  if (beams[at + SAMPLE_NODE + NODE_ZOOM] != SAMPLE_PARKED && !list_parked(item)) {
    return false;
  }
  beams[at + SAMPLE_NODE + NODE_ZOOM] = SAMPLE_PARKED;
  beams[at + SAMPLE_PARKED_T] = bitcast<u32>(t);
  beams[at + SAMPLE_NODE + NODE_CORNER] = corner.x;
  beams[at + SAMPLE_NODE + NODE_CORNER + 1u] = corner.y;
  beams[at + SAMPLE_NODE + NODE_CORNER + 2u] = corner.z;
  beams[at + SAMPLE_PARKED_STATE] = parked_word(zoom, roots, step);
  return true;
}

fn parked(item: u32) -> bool {
  return beams[sample_word(sample_of(item)) + SAMPLE_NODE + NODE_ZOOM] == SAMPLE_PARKED;
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

fn finish(item: u32, aim: Aim, w: WalkEnd) {
  sample_put(sample_word(sample_of(item)), w);
}
