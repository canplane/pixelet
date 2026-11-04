// pxl-render/hit_faces.wgsl

// Each hit the walks again kept, a thread each as `hit_lead_cs`'s, once the faces' pass
// asked what lies about each patch's face (pxl-shader raster.rs `face_cs`): those faces put
// at its pixel as its last ray word (pxl-engine `ray::FACES`), its own where it asked them,
// else those of the hit that asked for it, reached through the neighbors each was told
// (`beam::HIT_FOLLOWS`), a step left or up within the square each, so two of its sides at
// most; no pixel is left waiting past the cast's pass (pxl-shader cast.rs `hit_faces_cs`).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
@group(0) @binding(8) var<storage, read> beams: array<u32>;
@group(0) @binding(9) var<storage, read> pyramid: array<u32>;

@compute @workgroup_size(HIT_LANES)
fn hit_faces_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= min(pyramid[HITS_CNT], HITS)) {
    return;
  }
  let k = hit_at(id.x);
  let px = beams[k + HIT_PX];
  var faces = beams[k + HIT_OUT];
  for (var steps = 0u; (faces & HIT_FOLLOWS) != 0u && steps < 2u * HIT_SQUARE; steps += 1u) {
    faces = beams[hit_at(faces & ~HIT_FOLLOWS) + HIT_OUT];
  }
  textureStore(rays, vec2<u32>(px & 0xffffu, px >> 16u), RAY_FACES, vec4<u32>(faces, 0u, 0u, 0u));
}
