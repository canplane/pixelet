// The atmosphere's uniform, its transmittance table and the sampler every
// table is read with, at group 0 from binding 8: past the frame's own and
// the chunk's, so one numbering serves every pass that reads the air — the
// tables' own passes, the sky, and each lit pass's shading.

// Held as vec4 words, each field read from its own by `atmos_<field>()`:
// what the host and a shader share has no room inside it for a driver to
// lay out its own way (issue/20260927-a-matrix-the-driver-reads-its-own-way.md),
// and nothing builds the struct back, with which Safari's WebGPU lost its
// GPU process.
@group(0) @binding(8) var<uniform> atmos_words: array<vec4f, 14>;
@group(0) @binding(9) var transmittance_lut: texture_2d<f32>;
@group(0) @binding(11) var atmos_sampler: sampler;
