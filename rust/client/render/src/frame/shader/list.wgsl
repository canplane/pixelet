// pxl-render/list.wgsl

// A list walked apart and the walk over it: its dispatch and count lie in `args`
// (light_stood.wgsl `Stood`), its entries where its pass keeps them. A list is walked
// `MISS_LANES` a workgroup, `MISS_ACROSS` workgroups a row (pxl-engine `ray::LEFT_EMPTY`).

/// The place in its list of the thread at lane `lane` of workgroup `group`.
fn list_place(group: vec2<u32>, lane: u32) -> u32 {
  return (group.y * MISS_ACROSS + group.x) * MISS_LANES + lane;
}

/// One more listed in the list of `args`: its place, the rows of the walk over it raised by
/// the place that opens each, none past `room` (`workgroups`).
fn enlist(room: u32) -> u32 {
  let place = atomicAdd(&args.cnt, 1u);
  let per = MISS_LANES * MISS_ACROSS;
  if (place < room && place % per == 0u) {
    atomicMax(&args.list[1], workgroups(place + 1u, room, per));
  }
  return place;
}
