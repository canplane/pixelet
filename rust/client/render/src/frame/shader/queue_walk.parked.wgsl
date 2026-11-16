// pxl-render/frame/shader/queue_walk.parked.wgsl

// The run's second walk (queue_walk.wgsl): it takes the rays the first walk parked, each from
// the state it was parked in, and counts its asks for the asks after it (`QUEUE_ASKS_PARKED`,
// queue_ask.wgsl `queue_ask_parked_cs`).

fn queue_next() -> u32 {
  return next_parked();
}

fn queue_start(item: u32, aim: Aim) -> Start {
  return parked_start(item);
}

fn queue_asks_at() -> u32 {
  return QUEUE_ASKS_PARKED;
}

@compute @workgroup_size(QUEUE_LANE_CNT)
fn queue_parked_cs() {
  queue_walk();
}
