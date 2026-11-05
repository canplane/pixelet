// pxl-render/decor_mark.wgsl

// The eye's entries whose decor generates again this frame, a thread an entry, each decor shown
// at its clip level listed: whole where every entry does or its records were not generated
// for its brick, its head then laid anew and each decor's count none yet; else in part, from
// the list's end, where a box of the frame's reaches what its brick reads
// (pxl-content `generation::LIST_AT`, `part_at`).

@group(0) @binding(9) var<storage, read_write> records: array<u32>;
/// What the pass lists (pxl-content `generation::THIN_ARGS`): the words before the boxes
/// atomic, the counts its threads add to among them; the boxes and the pairs plain. The
/// boxes are the host's, only read here; a pair is written by the one thread whose add took
/// its place, and read by the passes after.
struct List {
  head: array<atomic<u32>, BOXES_AT>,
  words: array<u32>,
}

@group(0) @binding(11) var<storage, read_write> list: List;
@group(0) @binding(12) var<storage, read> shown: array<u32>;

fn list_word(k: u32) -> u32 {
  if (k < BOXES_AT) {
    return atomicLoad(&list.head[k]);
  }
  return list.words[k - BOXES_AT];
}

@compute @workgroup_size(LANES)
fn decor_mark_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let s = id.x;
  let row = clipmap[s];
  if (row.z == 0i) {
    return;
  }
  let cell = FIRST_CELL + s / CLIP_ENTRIES;
  let whole = list_word(LIST_EVERY) != 0u || !generated_for(s, row.xy);
  if (whole) {
    let h = s * HEAD_WORDS;
    records[h] = u32(row.x);
    records[h + 1u] = u32(row.y);
    records[h + 2u] = 1u;
    for (var k = 0u; k < DECORS; k += 1u) {
      records[cnt_at(s, k)] = 0u;
    }
  } else {
    let k = BRICK_SHIFT + LEAF_CELL - cell;
    let cells = vec4<i32>(row.xy << vec2<u32>(k), (row.xy + 1i) << vec2<u32>(k));
    if (!is_changed(cell, cells)) {
      return;
    }
  }
  // The pairs listed whole and in part together take no more places than the list holds,
  // so the two never meet.
  let room = LIST_WORDS - LIST_AT;
  for (var j = 0u; j < DECORS; j += 1u) {
    if (((shown[j] >> (cell - FIRST_CELL)) & 1u) != 0u) {
      let all = atomicAdd(&list.head[GENERATE_CNT], 1u);
      if (all >= room) {
        continue;
      }
      atomicMax(&list.head[GENERATE_ARGS + 2u], workgroups(all + 1u, room, 1u));
      var w = 0u;
      if (whole) {
        let n = atomicAdd(&list.head[THIN_CNT], 1u);
        atomicMax(&list.head[THIN_ARGS + 2u], workgroups(n + 1u, room, 1u));
        w = LIST_AT + n;
      } else {
        w = part_at(atomicAdd(&list.head[LIST_PARTS], 1u));
      }
      list.words[w - BOXES_AT] = pair_word(s, j);
    }
  }
}
