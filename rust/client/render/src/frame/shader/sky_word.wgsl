// The sky's own words. Their fields and accessors come from frame/shader/word.rs `sky`, which
// build.rs writes here.
@group(0) @binding(19) var<uniform> sky_words: array<vec4f, SKY_VEC4S>;
