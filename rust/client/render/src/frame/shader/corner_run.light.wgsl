// pxl-render/corner_run.light.wgsl

// What the queue's passes over the light's beam rays read of their run (queue_walk.wgsl,
// queue_ask.wgsl, queue_last.wgsl, corner.wgsl): the tag their walks put in the node cache's
// keys, where their asks' records lie, and the lanes their visits and asks are counted in.

fn walk_pass() -> u32 {
  return LIGHT_BEAM_PASS;
}

/// Where the asks' records lie in the run's view of its queue (pxl-engine
/// `ray::queue_recs_at`, `ray::QUEUE_LIGHT_CORNERS`).
fn queue_recs_at() -> u32 {
  return LIGHT_CORNERS_RECS_AT;
}

/// Where the run lists the items it parked (pxl-engine `light::parked_items_at`).
fn parked_items_at() -> u32 {
  return header_word(LIGHT_HEADER_PARKED_ITEMS);
}

fn visits_at() -> u32 {
  return LIGHT_BEAM_CNT;
}

fn asked_at() -> u32 {
  return LIGHT_ASKED_CNT;
}
