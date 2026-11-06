// pxl-render/holes_run.wgsl

// The holes' run, either observer's, as the queue's passes count its work (queue_walk.wgsl,
// queue_ask.wgsl, queue_last.wgsl): its walks' tag in the node cache's keys, its kind's caps,
// and the lanes its visits and asks are counted in, the observer the bound head's
// (light_aim.wgsl `light_run`).

const WALK_PASS: u32 = MISSED_PASS;

/// The first of its kind's caps in the node cache's head (pxl-engine `node_cache::caps_at`,
/// `ray::EYE_HOLES`, `ray::LIGHT_HOLES`).
fn queue_caps() -> u32 {
  return select(EYE_HOLES_CAPS, LIGHT_HOLES_CAPS, light_run());
}

/// The lanes of the nodes its walks visit and ask (pxl-engine `count::MISS`, `count::ASKED`).
fn visits_at() -> u32 {
  return select(EYE_MISS_COUNT, LIGHT_MISS_COUNT, light_run());
}

fn asked_at() -> u32 {
  return select(EYE_ASKED_COUNT, LIGHT_ASKED_COUNT, light_run());
}
