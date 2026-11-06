// pxl-render/sample_box.wgsl

// An observer's samples as stops into its record of voxels (record_put.wgsl `upsert`;
// pxl-engine record.rs), the eye's and the light's alike, each as its sample.eye.wgsl or
// sample.light.wgsl and its sample_box.eye.wgsl or sample_box.light.wgsl take it, a thread a
// sample, in two passes: the first (`sample_box_cs`) has each sample that heads its node's
// run, no sample before it across or up stopped on the node, find its entry or claim and
// write it; the second (`sample_observe_cs`), once every write of the first is done, has
// every stopped sample find its entry; a head that met its key's claim in the first is taken
// there, with every other sample, past the dispatch's end. Each marks its entry seen and sets
// the face it entered across, asking that face's word the first time where its observer reads
// one (`sample_asks`).

/// Whether sample `s` stopped on a node.
fn stopped(s: vec2<u32>) -> bool {
  return beams[sample_word(s) + SAMPLE_NODE + NODE_ZOOM] != SAMPLE_NONE;
}

/// Whether samples `a` and `b` stopped on one node.
fn same_node(a: vec2<u32>, b: vec2<u32>) -> bool {
  let ka = sample_word(a) + SAMPLE_NODE;
  let kb = sample_word(b) + SAMPLE_NODE;
  for (var i = 0u; i < NODE_WORDS; i += 1u) {
    if (beams[ka + i] != beams[kb + i]) {
      return false;
    }
  }
  return true;
}

/// Sample `s`'s node as the record keys it, its roots from where its ray starts.
fn sample_kept(s: vec2<u32>) -> Kept {
  return kept_at(sample_word(s) + SAMPLE_NODE, sample_band(s), sample_aim(s).origin);
}

/// Sample `s`'s stop: its answer, the face it entered across, and the line its cell's rays
/// resumed from (`record_put.wgsl` `Stop`).
fn sample_stop(s: vec2<u32>) -> Stop {
  let w = sample_word(s);
  let axis = beams[w + SAMPLE_AXIS];
  let dir = sample_aim(s).dir;
  let along = select(select(dir.z, dir.y, axis == 1u), dir.x, axis == 0u);
  return Stop(
    vec2<u32>(beams[w + SAMPLE_ASSET], beams[w + SAMPLE_ASSET + 1u]),
    beams[w + SAMPLE_LACKS] != 0u,
    sample_whole(s, sample_kept(s)),
    2u * axis + select(0u, 1u, along > 0.0),
    sample_line(s),
  );
}

/// The sample the thread at `id` takes, where the lattice holds it and it stopped.
fn stopped_item(id: vec2<u32>) -> vec2<u32> {
  let n = sample_lattice();
  let s = sample_item(id);
  if (s.x >= n.x || s.y >= n.y || !stopped(s)) {
    return vec2<u32>(NO_ENTRY);
  }
  return s;
}

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn sample_box_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let s = stopped_item(id.xy);
  if (s.x == NO_ENTRY) {
    return;
  }
  if ((s.x > 0u && same_node(s, s - vec2<u32>(1u, 0u))) || (s.y > 0u && same_node(s, s - vec2<u32>(0u, 1u)))) {
    return;
  }
  _ = upsert(sample_kept(s), sample_stop(s), true, sample_asks());
}

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn sample_observe_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let s = stopped_item(id.xy);
  if (s.x == NO_ENTRY) {
    return;
  }
  _ = upsert(sample_kept(s), sample_stop(s), false, sample_asks());
}
