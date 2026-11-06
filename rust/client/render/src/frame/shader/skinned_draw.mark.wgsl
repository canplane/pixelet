// pxl-render/frame/shader/skinned_draw.mark.wgsl

// An actor's mark's draw: the mark's own uniform at a dynamic offset, as it
// was or as it is (gpu/pass/mark.rs `Marks`), whatever instance it is.
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, DRAW_VEC4S>;

fn draw_at(instance: u32) {}

fn draw_word(k: u32) -> vec4f { return draw_words[k]; }
