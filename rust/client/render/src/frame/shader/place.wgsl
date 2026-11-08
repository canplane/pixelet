// `place_up` turns glTF's Y-up to Z-up, taking (x, y, z) to (x, -z, y), and then
// spins the result about Z.

fn spin_z(v: vec3f, angle: f32) -> vec3f {
  let c = cos(angle);
  let s = sin(angle);
  return vec3f(v.x * c - v.y * s, v.x * s + v.y * c, v.z);
}

fn place_up(v: vec3f, spin: f32) -> vec3f {
  return spin_z(vec3f(v.x, -v.z, v.y), spin);
}
