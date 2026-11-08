// atomicCompareExchangeWeak alone, as pxl-shader pyramid.rs `pyramid_swap`
// and way.rs `pop` reach it.
@group(0) @binding(0) var<storage, read_write> words: array<atomic<u32>>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let r = atomicCompareExchangeWeak(&words[id.x], 0u, id.x + 1u);
    if r.exchanged {
        atomicStore(&words[id.x + 64u], r.old_value);
    }
}
