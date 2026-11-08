// pxl-render/list.wgsl

// A list that a pass fills and a later dispatch walks, one thread for each entry. The list's
// count and the walk's dispatch lie in `args` (light_receiver_tiles.wgsl `ReceiverArgs`), and
// its entries lie wherever its pass keeps them. The walk runs `MISS_LANES` threads a workgroup
// and `MISS_ACROSS` workgroups a row, and its dispatch starts with no rows (pxl-engine
// `ray::LEFT_EMPTY`).

/// The place in the list that the thread at lane `lane` of workgroup `group` takes.
fn list_place(group: vec2<u32>, lane: u32) -> u32 {
  return (group.y * MISS_ACROSS + group.x) * MISS_LANES + lane;
}

/// Lists one more entry in the list of `args` and returns its place, which may lie past `cap`.
/// The entry whose place opens a row of the walk raises the dispatch's rows to hold it. A place
/// past `cap` opens no row, so the walk never reads past the list's room (`workgroups`).
fn enlist(cap: u32) -> u32 {
  let place = atomicAdd(&args.cnt, 1u);
  let per = MISS_LANES * MISS_ACROSS;
  if (place < cap && place % per == 0u) {
    atomicMax(&args.list[1], workgroups(place + 1u, cap, per));
  }
  return place;
}
