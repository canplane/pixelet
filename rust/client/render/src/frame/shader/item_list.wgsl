// pxl-render/item_list.wgsl

// The items of a kind laid as a list (samples.wgsl, corners.wgsl), as the queue's walks take
// them (queue_walk.wgsl, queue_last.wgsl): each walk the list's next place off its counter.

/// The next item a walk over the node cache takes, the first or the one `again`; `NONE` past the
/// list.
fn next_item(again: bool) -> u32 {
  return list_item(take(select(QUEUE_NEXT, QUEUE_NEXT_AGAIN, again)));
}

/// The next item the last walk takes; `NONE` past the list.
fn next_last() -> u32 {
  return list_item(take(LAST_NEXT));
}

/// The item at place `k`, `NONE` past the list.
fn list_item(k: u32) -> u32 {
  if (k >= item_count()) {
    return NONE;
  }
  return item_at(k);
}
