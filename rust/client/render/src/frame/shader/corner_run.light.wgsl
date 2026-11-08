// pxl-render/corner_run.light.wgsl

// How the queue's passes over the light's beam rays count their work (queue_walk.wgsl,
// queue_ask.wgsl, queue_last.wgsl, corner.wgsl): the tag their walks put in the node cache's
// keys, and the lanes their visits and asks are counted in.

const WALK_PASS: u32 = LIGHT_BEAM_PASS;

/// Where the asks' records lie in the run's view of its queue (pxl-engine
/// `ray::queue_records_at`, `ray::QUEUE_LIGHT_CORNERS`).
fn queue_records_at() -> u32 {
  return LIGHT_CORNERS_RECORDS_AT;
}

fn visits_at() -> u32 {
  return LIGHT_BEAM_COUNT;
}

fn asked_at() -> u32 {
  return LIGHT_ASKED_COUNT;
}
