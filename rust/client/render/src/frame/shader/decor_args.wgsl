// pxl-render/frame/shader/decor_args.wgsl

// Lays each region after the one before, from `decor_cnt_cs`'s counts. A region's first slot
// is kept for `decor_compact_cs` and added to its draw uniform's index. Its indirect args draw a
// model region indexed by its group's mesh, and an impostor region as a strip an instance, as
// many instances as the decor's instances hold past its first slot (pxl-content
// `generation::drawing`).

@group(0) @binding(0) var<storage, read> meshes: array<u32>;
@group(0) @binding(1) var<storage, read_write> regions: array<u32>;
@group(0) @binding(2) var<storage, read_write> args: array<u32>;
@group(0) @binding(3) var<storage, read_write> uniforms: array<u32>;

@compute @workgroup_size(1)
fn decor_args_cs() {
  var first = 0u;
  let cap = instance_cap();
  for (var r = 0u; r < REGION_CNT; r += 1u) {
    let n = regions[r];
    regions[REGION_CNT + r] = first;
    uniforms[r * UNIFORM_WORDS + UNIFORM_IDX] += first;
    let a = ARGS_WORDS * r;
    args[a] = select(IMPOSTOR_CORNER_CNT, meshes[r / 2u], (r & 1u) == 0u);
    args[a + 1u] = in_cap(n, cap - min(first, cap));
    args[a + 2u] = 0u;
    args[a + 3u] = 0u;
    args[a + 4u] = 0u;
    first += n;
  }
}
