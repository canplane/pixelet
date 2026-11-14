// pxl-render/sample_hit.wgsl

// Writes an observer's samples into the record as stops, the eye's and the light's alike, one
// thread a sample (record_put.wgsl `observe_hit`; pxl-engine record.rs). Each sample that
// stopped finds its node's entry, which the walk that stopped it kept, and sets the face it
// entered across. Where its observer reads faces' words, the first sample to enter a face asks
// that face's word (`sample_asks`). What differs by observer comes from sample.eye.wgsl or
// sample.light.wgsl, and from sample_hit.eye.wgsl or sample_hit.light.wgsl.

/// Whether sample `s` stopped on a node: it neither went on to its reach nor is parked,
/// waiting in the queue (pxl-engine `beam::SAMPLE_PARKED`).
fn hits(s: vec2<u32>) -> bool {
  let zoom = beams[sample_word(s) + SAMPLE_NODE + NODE_ZOOM];
  return zoom != SAMPLE_NONE && zoom != SAMPLE_PARKED;
}

/// The node sample `s` stopped on, as a walk from where its ray starts holds it, with its roots
/// counted over the whole world.
fn sample_node(s: vec2<u32>) -> Node {
  return node_at(sample_word(s) + SAMPLE_NODE, sample_aim(s).origin);
}

/// What sample `s` found where it stopped (record_put.wgsl `Hit`). The sample keeps only the
/// axis it entered across. Its face is the axis's low side where its ray runs up the axis, and
/// the high side otherwise. Its line is the one its cell's rays resumed from.
fn sample_hit(s: vec2<u32>) -> Hit {
  let w = sample_word(s);
  let axis = beams[w + SAMPLE_AXIS];
  let dir = sample_aim(s).dir;
  let along = select(select(dir.z, dir.y, axis == 1u), dir.x, axis == 0u);
  return Hit(
    vec2<u32>(beams[w + SAMPLE_ASSET], beams[w + SAMPLE_ASSET + 1u]),
    (beams[w + SAMPLE_LACK_WORD] & SAMPLE_LACK) != 0u,
    sample_whole(s, sample_node(s)),
    2u * axis + select(0u, 1u, along > 0.0),
    sample_line(s),
  );
}

/// The sample the thread at `id` takes, or `NO_ENTRY` where the lattice does not hold it or
/// it did not stop.
fn hit_item(id: vec2<u32>) -> vec2<u32> {
  let n = sample_lattice();
  let s = sample_item(id);
  if (s.x >= n.x || s.y >= n.y || !hits(s)) {
    return vec2<u32>(NO_ENTRY);
  }
  return s;
}

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn sample_hit_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let s = hit_item(id.xy);
  if (s.x == NO_ENTRY) {
    return;
  }
  observe_hit(sample_node(s), sample_hit(s), sample_asks());
}
