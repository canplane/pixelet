// pxl-render/corners_run.eye.wgsl

// The eye's beam rays' run as the queue's passes count its work (queue_walk.wgsl,
// queue_ask.wgsl, queue_last.wgsl; corners.wgsl): its walks' tag in the node cache's keys, the
// eye's lattice's caps, and the lanes its visits and asks are counted in.

const WALK_PASS: u32 = BEAM_PASS;

/// The first of the eye's lattice's caps in the node cache's head (pxl-engine
/// `node_cache::caps_at`, `count::EYE_LATTICE`).
fn queue_caps() -> u32 {
  return EYE_LATTICE_CAPS;
}

fn visits_at() -> u32 {
  return EYE_BEAM_COUNT;
}

fn asked_at() -> u32 {
  return EYE_ASKED_COUNT;
}
