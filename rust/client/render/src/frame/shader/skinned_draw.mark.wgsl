// pxl-render/frame/shader/skinned_draw.mark.wgsl

// A character's draw in the mark pass reads the mark's own uniform at a dynamic offset, as the
// character was or as it is (gpu/pass/mark.rs `Marks`). The instance names no draw here.
@group(1) @binding(0) var<uniform> skinned_words: array<vec4f, DRAW_VEC4S>;

fn draw_at(instance: u32) {}
