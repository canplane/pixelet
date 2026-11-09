// pxl-render/hole_run.wgsl

// The holes' run, of the eye's pixels or of the light's slots, as the queue's passes count its
// work (queue_walk.wgsl, queue_ask.wgsl, queue_last.wgsl). It gives the tag its walks write in
// the node cache's keys, and the count lanes its visits and asks add to. The observer is read
// off the light's head that is bound, which is all zero for the eye (light_aim.wgsl
// `light_run`).

fn walk_pass() -> u32 {
  return MISSED_PASS;
}

/// Where the asks' records lie, counted from the first word of the observer's holes' queue
/// (pxl-engine `ray::queue_records_at` of `ray::QUEUE_EYE_HOLES` or `ray::QUEUE_LIGHT_HOLES`).
fn queue_records_at() -> u32 {
  return select(EYE_HOLES_RECORDS_AT, LIGHT_HOLES_RECORDS_AT, light_run());
}

/// The lane in which the run's walks count the nodes they visit (pxl-engine `count::HOLE_WALK`).
/// `asked_at` is the lane in which they count the nodes they ask (`count::ASKS`).
fn visits_at() -> u32 {
  return select(EYE_MISS_CNT, LIGHT_MISS_CNT, light_run());
}

fn asked_at() -> u32 {
  return select(EYE_ASKED_CNT, LIGHT_ASKED_CNT, light_run());
}
