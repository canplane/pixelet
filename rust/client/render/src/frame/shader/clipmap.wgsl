// pxl-render/frame/shader/clipmap.wgsl

// Binds the clip levels' table (pxl-engine `clipmap::Entry`) for a pass that only reads it.

@group(0) @binding(1) var<storage, read> clipmap: array<vec4<i32>>;
