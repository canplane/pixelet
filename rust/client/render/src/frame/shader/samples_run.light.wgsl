// pxl-render/samples_run.light.wgsl

// The light's samples' run as the queue's passes count its work (queue_walk.wgsl,
// queue_ask.wgsl, queue_last.wgsl; samples.wgsl): its walks' tag in the node cache's keys, and
// the lanes its visits and asks are counted in.

const WALK_PASS: u32 = LIGHT_SAMPLES_PASS;

/// Where the asks' records lie in its queue's view (pxl-engine `ray::queue_records_at`,
/// `ray::QUEUE_LIGHT_SAMPLES`).
fn queue_records_at() -> u32 {
  return LIGHT_SAMPLES_RECORDS_AT;
}

fn visits_at() -> u32 {
  return LIGHT_SAMPLE_COUNT;
}

fn asked_at() -> u32 {
  return LIGHT_ASKED_COUNT;
}
