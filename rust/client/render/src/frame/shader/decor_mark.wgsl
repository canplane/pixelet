// pxl-render/decor_mark.wgsl

// Lists the eye's entries whose decor generates again this frame, one thread an entry, with a
// pair for each decor shown at the entry's clip level. An entry is listed whole when every entry
// generates again or its records were not generated for its brick; its header is then laid anew
// and each decor's count set to none. Otherwise it is listed in part, from the list's end, when
// a box of the frame's reaches what its brick reads (pxl-content `generation::LIST_AT`,
// `part_at`).

@group(0) @binding(9) var<storage, read_write> recs: array<u32>;
/// What the pass lists (pxl-content `generation::THIN_ARGS`). The words before the boxes are
/// atomic, as the counts the threads add to lie among them. The boxes and the pairs are plain:
/// the host writes the boxes and this pass only reads them, and each pair is written by the one
/// thread whose add took its place and read by the passes after.
struct List {
  header: array<atomic<u32>, RECTS_AT>,
  words: array<u32>,
}

@group(0) @binding(11) var<storage, read_write> list: List;
@group(0) @binding(12) var<storage, read> shown: array<u32>;

fn list_word(k: u32) -> u32 {
  if (k < RECTS_AT) {
    return atomicLoad(&list.header[k]);
  }
  return list.words[k - RECTS_AT];
}

@compute @workgroup_size(LANES)
fn decor_mark_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let s = id.x;
  let row = clipmap[s];
  if (row.z == 0i) {
    return;
  }
  let cell_zoom = FIRST_CELL_ZOOM + s / CLIP_ENTRIES;
  let whole = list_word(LIST_EVERY) != 0u || !generated_for(s, row.xy);
  if (whole) {
    let h = s * HEADER_WORDS;
    recs[h] = u32(row.x);
    recs[h + 1u] = u32(row.y);
    recs[h + 2u] = 1u;
    for (var k = 0u; k < DECORS; k += 1u) {
      recs[cnt_at(s, k)] = 0u;
    }
  } else {
    let k = BRICK_SHIFT + LEAF_CELL_ZOOM - cell_zoom;
    let cells = vec4<i32>(row.xy << vec2<u32>(k), (row.xy + 1i) << vec2<u32>(k));
    if (!is_changed(cell_zoom, cells)) {
      return;
    }
  }
  // The pairs listed whole fill the list from its start, and those listed in part from its
  // end. Together they take no more places than the list holds, so the two never meet.
  let cap = LIST_WORDS - LIST_AT;
  for (var j = 0u; j < DECORS; j += 1u) {
    if (((shown[j] >> (cell_zoom - FIRST_CELL_ZOOM)) & 1u) != 0u) {
      let all = atomicAdd(&list.header[GENERATE_CNT], 1u);
      if (all >= cap) {
        continue;
      }
      atomicMax(&list.header[GENERATE_ARGS + 2u], workgroups(all + 1u, cap, 1u));
      var w = 0u;
      if (whole) {
        let n = atomicAdd(&list.header[THIN_CNT], 1u);
        atomicMax(&list.header[THIN_ARGS + 2u], workgroups(n + 1u, cap, 1u));
        w = LIST_AT + n;
      } else {
        w = part_at(atomicAdd(&list.header[LIST_PARTS], 1u));
      }
      list.words[w - RECTS_AT] = pair_word(s, j);
    }
  }
}
