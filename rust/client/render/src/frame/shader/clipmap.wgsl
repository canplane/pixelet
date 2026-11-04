// pxl-render/clipmap.wgsl

// The clip levels' table (pxl-engine `clipmap::Entry`), as a pass that only reads it binds it.

@group(0) @binding(1) var<storage, read> clipmap: array<vec4<i32>>;
