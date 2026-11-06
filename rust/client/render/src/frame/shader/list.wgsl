// pxl-render/list.wgsl

// A list of rays walked apart, the eye's or the light's, and the walk over it: a list's
// dispatch and count lie in `args`, the walk again's (`MissArgs`) or what a walk over the node
// cache left (`Left`), its entries where its pass keeps them. Every list is walked
// `MISS_LANES` a workgroup, `MISS_ACROSS` workgroups a row (pxl-engine `ray::LEFT_EMPTY`).

/// The walk again's words (pxl-engine `ray::MISS_ARGS_WORDS`): its dispatch over the list,
/// its rows raised as the list fills (`MISS_ARGS_LIST`); the count listed (`MISS_ARGS_CNT`);
/// and the counts' lanes (`MISS_ARGS_COUNT`). build.rs holds its words to pxl-engine's.
struct MissArgs {
  list: array<atomic<u32>, 3>,
  cnt: atomic<u32>,
  counts: array<atomic<u32>, COUNT_RAYS>,
}

/// What a walk over the node cache left (pxl-engine `ray::LEFT_EMPTY`): the dispatch of the walk
/// over it, then its count (`ray::LEFT_CNT`). build.rs holds its words to pxl-engine's.
struct Left {
  list: array<atomic<u32>, 3>,
  cnt: atomic<u32>,
}

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
