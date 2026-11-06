// pxl-render/corners_run.light.wgsl

// The light's beam rays' run as the queue's passes count its work (queue_walk.wgsl,
// queue_ask.wgsl, queue_last.wgsl; corners.wgsl): its walks' tag in the node cache's keys, the
// light's lattice's caps, and the lanes its visits and asks are counted in.

const WALK_PASS: u32 = LIGHT_BEAM_PASS;

/// The first of the light's lattice's caps in the node cache's head (pxl-engine
/// `node_cache::caps_at`, `count::LIGHT_LATTICE`).
fn queue_caps() -> u32 {
  return LIGHT_LATTICE_CAPS;
}

fn visits_at() -> u32 {
  return LIGHT_BEAM_COUNT;
}

fn asked_at() -> u32 {
  return LIGHT_ASKED_COUNT;
}
