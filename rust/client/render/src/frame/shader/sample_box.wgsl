// pxl-render/sample_box.wgsl

// An observer's samples as stops into what draws the node cache's entries (record_put.wgsl
// `observe_stop`; pxl-engine record.rs), the eye's and the light's alike, each as its
// sample.eye.wgsl or sample.light.wgsl and its sample_box.eye.wgsl or sample_box.light.wgsl
// take it, a thread a sample: every stopped sample finds its node's entry, which the walk that
// stopped it kept, and sets the face it entered across, asking that face's word the first time
// where its observer reads one (`sample_asks`).

/// Whether sample `s` stopped on a node: not gone on to its reach, and not parked
/// (`beam::SAMPLE_PARKED`).
fn stopped(s: vec2<u32>) -> bool {
  let zoom = beams[sample_word(s) + SAMPLE_NODE + NODE_ZOOM];
  return zoom != SAMPLE_NONE && zoom != SAMPLE_PARKED;
}

/// Sample `s`'s node, its roots world-absolute from where its ray starts.
fn sample_kept(s: vec2<u32>) -> Node {
  return kept_at(sample_word(s) + SAMPLE_NODE, sample_aim(s).origin);
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
  observe_stop(sample_kept(s), sample_stop(s), sample_asks());
}
