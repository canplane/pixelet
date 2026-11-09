// pxl-render/item_list.wgsl

// How the queue's walks (queue_walk.wgsl, queue_last.wgsl) take the items of a run that lists
// them: the samples (sample.wgsl) or the beam rays (corner.wgsl). The first walk takes the
// run's items in turn. A ray that parks lists its item once (`list_parked`), and the walks after
// it take only those.

/// The next item for a walk over the node cache to take: of the run's, or, in the `parked`
/// walk, of those the first parked. It is `NONE` past the list.
fn next_item(parked: bool) -> u32 {
  if (parked) {
    return parked_item(take_below(QUEUE_NEXT_PARKED, load(QUEUE_PARKED)));
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
  return parked_item(take_below(LAST_NEXT, load(LAST_CNT)));
}

fn parked_item(k: u32) -> u32 {
  if (k == NONE) {
    return NONE;
  }
  return beams[parked_items_at() + k];
}

/// Lists `item`, whose ray parks for the first time, for the walks after this one.
fn list_parked(item: u32) {
  beams[parked_items_at() + take(QUEUE_PARKED)] = item;
}
