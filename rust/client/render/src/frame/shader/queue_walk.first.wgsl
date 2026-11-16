// pxl-render/frame/shader/queue_walk.first.wgsl

// The run's first walk (queue_walk.wgsl): it takes the run's items, each from where its ray
// starts, and counts its asks for the asks after it (`QUEUE_ASKS`, queue_ask.wgsl `queue_ask_cs`).

fn queue_next() -> u32 {
  return next_item();
}

fn queue_start(item: u32, aim: Aim) -> Start {
  return item_start(item, aim);
}

fn queue_asks_at() -> u32 {
  return QUEUE_ASKS;
}

@compute @workgroup_size(QUEUE_LANE_CNT)
fn queue_cs() {
  queue_walk();
}
