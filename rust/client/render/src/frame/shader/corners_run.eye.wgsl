// pxl-render/corners_run.eye.wgsl

// The eye's beam rays' run as the queue's passes count its work (queue_walk.wgsl,
// queue_ask.wgsl, queue_last.wgsl; corners.wgsl): its walks' tag in the node cache's keys, and
// the lanes its visits and asks are counted in.

const WALK_PASS: u32 = BEAM_PASS;

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
