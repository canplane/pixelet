// pxl-render/item_list.wgsl

// How the queue's walks (queue_walk.wgsl, queue_last.wgsl) take the items of a run that lists
// them: the samples (sample.wgsl) or the beam rays (corner.wgsl). Each walk takes the next
// place in the list from a counter of its own.

/// The next item for a walk over the node cache to take, by the first walk's counter or, where
/// `again`, by the second walk's. It is `NONE` past the list.
fn next_item(again: bool) -> u32 {
  return list_item(take(select(QUEUE_NEXT, QUEUE_NEXT_AGAIN, again)));
}

/// The next item for the last walk, the one that asks the world, to take. It is `NONE` past
/// the list.
fn next_last() -> u32 {
  return list_item(take(LAST_NEXT));
}

fn list_item(k: u32) -> u32 {
  if (k >= item_count()) {
    return NONE;
  }
  return item_at(k);
}
