// pxl-render/sample_run.eye.wgsl

// What the queue's passes (queue_walk.wgsl, queue_ask.wgsl, queue_last.wgsl) read of the
// eye's samples' run: the tag its walks' claims carry in the node cache's keys (`walk_pass`),
// where its asks' records lie, and the lanes its visits and asks are counted in.

fn walk_pass() -> u32 {
  return SAMPLES_PASS;
}

/// The first word of the asks' records, counted from the first word of the run's queue's view
/// (pxl-engine `ray::queue_records_at`, `ray::QUEUE_EYE_SAMPLES`).
fn queue_records_at() -> u32 {
  return EYE_SAMPLES_RECORDS_AT;
}

/// Where the run lists the items it parked (pxl-engine `beam::Grid::asks_at`).
fn parked_items_at() -> u32 {
  return eye_word(GRID + GRID_ASKS_AT);
}

fn visits_at() -> u32 {
  return EYE_SAMPLE_CNT;
}

fn asked_at() -> u32 {
  return EYE_ASKED_CNT;
}
