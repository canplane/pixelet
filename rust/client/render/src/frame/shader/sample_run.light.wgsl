// pxl-render/sample_run.light.wgsl

// What the queue's passes (queue_walk.wgsl, queue_ask.wgsl, queue_last.wgsl) read of the
// light's samples' run: the tag its walks' claims carry in the node cache's keys (`walk_pass`),
// where its asks' records lie, and the lanes its visits and asks are counted in.

fn walk_pass() -> u32 {
  return LIGHT_SAMPLES_PASS;
}

/// The first word of the asks' records, counted from the first word of the run's queue's view
/// (pxl-engine `ray::queue_recs_at`, `ray::QUEUE_LIGHT_SAMPLES`).
fn queue_recs_at() -> u32 {
  return LIGHT_SAMPLES_RECS_AT;
}

/// Where the run lists the items it parked (pxl-engine `light::parked_items_at`).
fn parked_items_at() -> u32 {
  return hdr_word(LIGHT_HDR_PARKED_ITEMS);
}

fn visits_at() -> u32 {
  return LIGHT_SAMPLE_CNT;
}

fn asked_at() -> u32 {
  return LIGHT_ASKED_CNT;
}
