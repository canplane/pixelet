// Bindings from 8 on, after the frame's and the chunk's, so every pass that
// reads the air shares one numbering.

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(0) @binding(8) var<uniform> atmos_words: array<vec4f, 16>;
@group(0) @binding(9) var transmittance_lut: texture_2d<f32>;
@group(0) @binding(11) var atmos_sampler: sampler;
