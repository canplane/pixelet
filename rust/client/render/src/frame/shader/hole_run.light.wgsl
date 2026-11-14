// pxl-render/frame/shader/hole_run.light.wgsl

// What the queue's passes over the light's holes read of their run (queue_walk.wgsl,
// queue_ask.wgsl, queue_last.wgsl, queue.wgsl): the tag their walks put in the node cache's
// keys, where their asks' records lie, and the lanes their visits and asks are counted in.

fn walk_pass() -> u32 {
  return MISSED_PASS;
}

/// Where the asks' records lie in the run's view of its queue (pxl-engine
/// `ray::queue_recs_at`, `ray::QUEUE_LIGHT_HOLES`).
fn queue_recs_at() -> u32 {
  return LIGHT_HOLES_RECS_AT;
}

fn visits_at() -> u32 {
  return LIGHT_MISS_CNT;
}

fn asked_at() -> u32 {
  return LIGHT_ASKED_CNT;
}
