// Placing a model in the world: a spin about Z, and glTF's Y-up turned to
// this world's Z-up first — a quarter turn about X, (x, y, z) to (x, -z, y)
// — then the spin. Decor and the actors place by it; the beacon, drawn
// Z-up already, spins alone.

fn spin_z(v: vec3f, angle: f32) -> vec3f {
  let c = cos(angle);
  let s = sin(angle);
  return vec3f(v.x * c - v.y * s, v.x * s + v.y * c, v.z);
}

fn place_up(v: vec3f, spin: f32) -> vec3f {
  return spin_z(vec3f(v.x, -v.z, v.y), spin);
}
