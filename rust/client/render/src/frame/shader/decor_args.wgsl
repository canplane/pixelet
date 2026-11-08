// pxl-render/decor_args.wgsl

// Each region laid after the one before, off `decor_cnt_cs`'s counts: its first slot kept
// for `decor_compact_cs` and added to its draw uniform's index, and its indirect args: a
// model region's indexed by its group's mesh, an impostor region's a strip an instance, as many
// as the instances hold past its first slot (pxl-content `generation::drawn`).

@group(0) @binding(0) var<storage, read> meshes: array<u32>;
@group(0) @binding(1) var<storage, read_write> regions: array<u32>;
@group(0) @binding(2) var<storage, read_write> args: array<u32>;
@group(0) @binding(3) var<storage, read_write> uniforms: array<u32>;

@compute @workgroup_size(1)
fn decor_args_cs() {
  var first = 0u;
  let cap = instance_cap();
  for (var r = 0u; r < REGIONS; r += 1u) {
    let n = regions[r];
    regions[REGIONS + r] = first;
    uniforms[r * UNIFORM_WORDS + UNIFORM_IDX] += first;
    let a = ARGS_WORDS * r;
    args[a] = select(IMPOSTOR_CORNERS, meshes[r / 2u], (r & 1u) == 0u);
    args[a + 1u] = in_cap(n, cap - min(first, cap));
    args[a + 2u] = 0u;
    args[a + 3u] = 0u;
    args[a + 4u] = 0u;
    first += n;
  }
}
