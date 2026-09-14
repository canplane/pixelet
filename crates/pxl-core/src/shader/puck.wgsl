// The location puck's draw uniform, shared by its zone and its beacon
// (marker.rs): the translation, the fourth float of the module's uniform
// (the beacon's spin), the colour the glue paints the zone in — the UI's
// action colour, which is the glue's to know — and whether it is night.

struct Draw {
  translation: vec3f,
  fourth: f32,
  color: vec3f,
  night: f32,
};

@group(1) @binding(0) var<uniform> draw: Draw;
