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

/// Where the asks' records lie in its queue's view (pxl-engine `ray::queue_records_at`,
/// `ray::QUEUE_EYE_CORNERS`).
fn queue_records_at() -> u32 {
  return EYE_CORNERS_RECORDS_AT;
}

fn visits_at() -> u32 {
  return EYE_BEAM_COUNT;
}

fn asked_at() -> u32 {
  return EYE_ASKED_COUNT;
}

/// A beam ray its run leaves parked, which keeps the cast's offset from being counted
/// (`node_cache::LATTICE_LEFT`; roll.wgsl), so the casts go on and it goes on in the next
/// (beam.eye.wgsl `beams_resume`).
fn corner_owed() {
  atomicAdd(&node_cache[LATTICE_LEFT], 1u);
}
