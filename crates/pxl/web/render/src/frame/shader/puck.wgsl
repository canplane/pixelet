// The location puck's draw uniform, shared by its zone and its beacon
// (marker.rs): the translation, the fourth float of the module's uniform
// (the beacon's spin), the color the glue paints the zone in — the UI's
// action color, which is the glue's to know — and whether it is night.

struct Draw {
  translation: vec3f,
  fourth: f32,
  color: vec3f,
  night: f32,
};

// Held as vec4 words and read back by `draw()`: what the host and a shader
// share has no room inside it for a driver to lay out its own way
// (issue/20260927-a-matrix-the-driver-reads-its-own-way.md).
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, 2>;

fn draw() -> Draw {
  return Draw(
    draw_words[0].xyz,
    draw_words[0].w,
    draw_words[1].xyz,
    draw_words[1].w,
  );
}
