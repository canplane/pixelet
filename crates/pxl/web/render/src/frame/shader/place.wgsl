// glTF's Y-up turned to Z-up, (x, y, z) to (x, -z, y), then the spin about Z.

fn spin_z(v: vec3f, angle: f32) -> vec3f {
  let c = cos(angle);
  let s = sin(angle);
  return vec3f(v.x * c - v.y * s, v.x * s + v.y * c, v.z);
}

fn place_up(v: vec3f, spin: f32) -> vec3f {
  return spin_z(vec3f(v.x, -v.z, v.y), spin);
}
