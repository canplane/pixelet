// pxl-render/hole_run.eye.wgsl

// What the queue's passes over the eye's holes read of their run (queue_walk.wgsl,
// queue_ask.wgsl, queue_last.wgsl, queue.wgsl): the tag their walks put in the node cache's
// keys, where their asks' records lie, and the lanes their visits and asks are counted in.

fn walk_pass() -> u32 {
  return MISSED_PASS;
}

/// Where the asks' records lie in the run's view of its queue (pxl-engine
/// `ray::queue_recs_at`, `ray::QUEUE_EYE_HOLES`).
fn queue_recs_at() -> u32 {
  return EYE_HOLES_RECS_AT;
}

/// The lane in which the run's walks count the nodes they visit (pxl-engine `count::HOLE_WALK`).
/// `asked_at` is the lane in which they count the nodes they ask (`count::ASK`).
fn visits_at() -> u32 {
  return EYE_MISS_CNT;
}

fn asked_at() -> u32 {
  return EYE_ASKED_CNT;
}
