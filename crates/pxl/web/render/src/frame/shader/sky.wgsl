// The sky: environment.tsx's dome shader (b9e31004),
// drawn as one full-screen triangle behind everything. The dome was a
// sphere of radius SKY_DOME_SCALE centred on the world origin, coloured by
// the direction from the origin to the point a pixel's ray meets it; that
// point is found here by intersecting the ray with the same sphere, so the
// colours land where they did. The gradient, stars, clouds, sun and moon
// are that shader's, line for line; the output is graded, vignetted and
// encoded to sRGB, as the post stack did over the whole screen — the frame
// uniform is bound for the vignette alone.

struct Sky {
  inv_view_proj: mat4x4f,
  camera_pos: vec3f,
  radius: f32,
  top: vec3f,
  time: f32,
  middle: vec3f,
  star_intensity: f32,
  horizon: vec3f,
  az: f32,
  sun_dir: vec3f,
  _p0: f32,
  sun_glow: vec3f,
  _p1: f32,
  moon_dir: vec3f,
  _p2: f32,
};

@group(0) @binding(1) var<uniform> sky: Sky;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) ndc: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VertexOut {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
  var o: VertexOut;
  o.clip = vec4f(p, 1.0, 1.0);
  o.ndc = p;
  return o;
}

fn hash(q: vec2f) -> f32 {
  var p = fract(q * vec2f(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

fn value_noise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let a = hash(i);
  let b = hash(i + vec2f(1.0, 0.0));
  let c = hash(i + vec2f(0.0, 1.0));
  let d = hash(i + vec2f(1.0, 1.0));
  let u = f * f * (3.0 - 2.0 * f);
  return mix(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
}

fn fbm(q: vec2f) -> f32 {
  var p = q;
  var sum = 0.0;
  var amp = 0.5;
  for (var i = 0; i < 4; i++) {
    sum += amp * value_noise(p);
    p *= 2.02;
    amp *= 0.5;
  }
  return sum;
}

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  // The pixel's ray, and where it meets the dome.
  let far = sky.inv_view_proj * vec4f(v.ndc, 1.0, 1.0);
  let ray = normalize(far.xyz / far.w - sky.camera_pos);
  let b = dot(sky.camera_pos, ray);
  let c = dot(sky.camera_pos, sky.camera_pos) - sky.radius * sky.radius;
  let t = -b + sqrt(max(b * b - c, 0.0));
  let dir = normalize(sky.camera_pos + ray * t);
  let h = clamp(dir.z * 0.5 + 0.5, 0.0, 1.0);
  let az = atan2(dir.y, dir.x) - sky.az;

  var color = mix(sky.horizon, sky.middle, smoothstep(0.0, 0.4, h));
  color = mix(color, sky.top, smoothstep(0.3, 1.0, h));

  let horizon_glow = 1.0 - smoothstep(0.0, 0.22, abs(h - 0.08));
  color += sky.horizon * horizon_glow * 0.06;

  if (sky.star_intensity > 0.001) {
    let star_uv = vec2f(az * 26.0, h * 90.0);
    let star_seed = hash(floor(star_uv));
    let twinkle = 0.55 + 0.45 * sin(sky.time * 2.2 + star_seed * 62.0);
    let star = smoothstep(0.975, 0.995, star_seed) * twinkle;
    color += vec3f(star) * smoothstep(0.04, 0.3, h) * sky.star_intensity;
  }

  let cloud_uv = vec2f(az * 2.4, h * 5.0) + vec2f(sky.time * 0.02, sky.time * 0.004);
  let cloud_density = smoothstep(0.52, 0.82, fbm(cloud_uv * 1.6));
  let cloud_mask = smoothstep(0.04, 0.28, h) * (1.0 - smoothstep(0.72, 1.0, h));
  color = mix(color, mix(sky.horizon, vec3f(1.0), 0.55), cloud_density * cloud_mask * 0.5);

  let sun_dot = max(dot(dir, normalize(sky.sun_dir)), 0.0);
  color = mix(color, sky.sun_glow, pow(sun_dot, 6.0) * 0.35);
  color = mix(color, vec3f(1.0, 0.98, 0.9), smoothstep(0.997, 0.9992, sun_dot));
  let sun_rim = smoothstep(0.997, 0.9985, sun_dot) * (1.0 - smoothstep(0.9985, 0.9992, sun_dot));
  color = mix(color, vec3f(1.0, 0.95, 0.85), sun_rim * 0.6);

  let moon_axis = normalize(sky.moon_dir);
  let moon_dot = max(dot(dir, moon_axis), 0.0);
  color = mix(color, vec3f(0.85, 0.9, 1.0), pow(moon_dot, 20.0) * sky.star_intensity * 0.4);
  let moon_disc_mask = smoothstep(0.998, 0.9993, moon_dot) * sky.star_intensity;
  if (moon_disc_mask > 0.001) {
    let ref_up = select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 0.0, 1.0), abs(moon_axis.z) < 0.99);
    let m_right = normalize(cross(ref_up, moon_axis));
    let m_up = cross(moon_axis, m_right);
    let moon_angular_radius = 0.0374;
    let local = dir - moon_dot * moon_axis;
    let p = dot(local, m_right) / moon_angular_radius;
    let q = dot(local, m_up) / moon_angular_radius;
    let nz = sqrt(max(0.0, 1.0 - p * p - q * q));
    let sun_n = normalize(sky.sun_dir);
    let lit = p * dot(sun_n, m_right) + q * dot(sun_n, m_up) + nz * (-dot(sun_n, moon_axis));
    let lit_mask = smoothstep(-0.05, 0.05, lit);
    let strength = mix(0.06, 1.0, lit_mask);
    color = mix(color, mix(vec3f(0.20, 0.21, 0.26), vec3f(0.92, 0.94, 0.98), lit_mask), moon_disc_mask * strength);
  }

  color += (hash(v.clip.xy) - 0.5) * (1.0 / 255.0);
  return vec4f(finish(color, v.clip.xy), 1.0);
}
