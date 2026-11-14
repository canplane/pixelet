// pxl-render/frame/shader/ask_run.wgsl

// The run whose asks the pass serves (queue_ask.wgsl), told by the queue it is bound to: the
// view reaches from that queue's head to the end of the records (pxl-engine `ray::queue_at`).
// The tag its claims carry in the node cache's keys, where its asks' records lie and the lane
// its asks are counted in follow from that.

/// The queue bound (pxl-engine `ray::QUEUE_EYE_CORNERS` and the rest).
fn bound_queue() -> u32 {
  return (QUEUE_WORDS - arrayLength(&queue)) / QUEUE_HDR_STRIDE;
}

/// A queue's number is its run's tag, but for the light's holes, which take the eye's.
fn walk_pass() -> u32 {
  let q = bound_queue();
  return select(q, MISSED_PASS, q == QUEUE_LIGHT_HOLES);
}

/// The first word of the asks' records, counted from the first word of the bound view.
fn queue_recs_at() -> u32 {
  return arrayLength(&queue) - QUEUE_RECS_CAP * QUEUE_REC_WORDS;
}

fn asked_at() -> u32 {
  return select(EYE_ASKED_CNT, LIGHT_ASKED_CNT, bound_queue() >= QUEUE_LIGHT_CORNERS);
}
