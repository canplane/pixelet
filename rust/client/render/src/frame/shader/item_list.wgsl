// pxl-render/frame/shader/item_list.wgsl

// How the queue's walks (queue_walk.wgsl, queue_last.wgsl) take the items of a run that lists
// them: the samples (sample.wgsl) or the beam rays (corner.wgsl). The first walk takes the
// run's items in turn. A ray that parks lists its item once (`list_parked`), and the walks after
// it take only those. A run parks no more items than it has, so the list's room is the run's
// count of items (`item_cnt`), and every count read here is held to it.

/// The next item for a walk over the node cache to take: of the run's, or, in the `parked`
/// walk, of those the first parked. It is `NONE` past the list.
fn next_item(parked: bool) -> u32 {
  if (parked) {
    return parked_item(take_below(QUEUE_NEXT_PARKED, in_cap(load(QUEUE_PARKED), item_cnt())));
  }
  let k = take_below(QUEUE_NEXT, item_cnt());
  if (k == NONE) {
    return NONE;
  }
  return item_at(k);
}

/// The next parked item for the last walk, the one that asks the world, to take. It is `NONE`
/// past the list.
fn next_last() -> u32 {
  return parked_item(take_below(LAST_NEXT, in_cap(load(LAST_CNT), item_cnt())));
}

fn parked_item(k: u32) -> u32 {
  if (k == NONE) {
    return NONE;
  }
  return beams[parked_items_at() + k];
}

/// Lists `item`, whose ray parks for the first time, for the walks after this one. It returns
/// false where the list has no room.
fn list_parked(item: u32) -> bool {
  let k = take(QUEUE_PARKED);
  if (k >= item_cnt()) {
    return false;
  }
  beams[parked_items_at() + k] = item;
  return true;
}
