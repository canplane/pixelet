// pxl-render/corner_run.eye.wgsl

// What the queue's passes over the eye's beam rays read of their run (queue_walk.wgsl,
// queue_ask.wgsl, queue_last.wgsl, corner.wgsl): the tag their walks put in the node cache's
// keys, where their asks' records lie, and the lanes their visits and asks are counted in.

const WALK_PASS: u32 = BEAM_PASS;

/// Where the asks' records lie in the run's view of its queue (pxl-engine
/// `ray::queue_records_at`, `ray::QUEUE_EYE_CORNERS`).
fn queue_records_at() -> u32 {
  return EYE_CORNERS_RECORDS_AT;
}

fn visits_at() -> u32 {
  return EYE_BEAM_CNT;
}

fn asked_at() -> u32 {
  return EYE_ASKED_CNT;
}
