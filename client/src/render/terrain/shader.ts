// @pixelet/client/render/terrain/shader.ts

import { GROUND_CH_KEYS, GroundChannelKey } from "@pixelet/shared/bake/format.ts";
import * as meta from "./shader_meta.ts";

// vertex shader
export const vert = /* glsl */ `
  precision highp float;
  precision highp int;
  precision highp isampler2D;
  #include <fog_pars_vertex>

  out vec2 v_uv;
  out vec2 v_box_uv;
  out float v_is_side_face;
  out float v_is_skirt;
  out float v_view_dist;
  out vec3 v_view_pos;
  out float v_surface_z;
  out vec2 v_world_xy;

  uniform isampler2D u_heightmap;
  uniform float u_scale;
  uniform float u_row_stride;
  uniform float u_lod_step;

  void main() {
    int i_lod_res = int(u_row_stride / u_lod_step);

    int ix_lod = gl_InstanceID % i_lod_res;
    int iy_lod = gl_InstanceID / i_lod_res;

    // ---- 1. 인덱스 및 샘플링 좌표 (tex_coord) ----

    int ix = ix_lod * int(u_lod_step);
    int iy = iy_lod * int(u_lod_step);
    vec2 tex_coord = vec2(float(ix), float(iy));
    
    v_uv = (tex_coord + 0.5) / u_row_stride;
    v_box_uv = uv;

    // ---- 2. 고도 데이터 샘플링 (surface_z) ----

    // 1. Z축 양자화 (LOD 계단화)
    int h = texelFetch(u_heightmap, ivec2(ix, iy), 0).r;
    h = (h / int(u_lod_step)) * int(u_lod_step); // Z-Quantization: 데이터 레벨의 Z-LOD 적용

    // 2. 물리적 높이 계산 및 팽창
    float surface_z = float(h) * u_scale; // 실제 월드의 고도(윗면)
    float visual_scale = u_scale * u_lod_step; // 복셀 한 칸의 덩치 (X,Y,Z 일괄 팽창)

    // ---- 3. 인스턴스 위치 (instance_pos) ----

    // 3. Z축 시프팅 (위치 이동)
    float offset = float(i_lod_res) * 0.5;
    vec3 instance_pos = vec3(
      (float(ix_lod) - offset + 0.5) * visual_scale,
      (float(iy_lod) - offset + 0.5) * visual_scale,
      surface_z - (visual_scale * 0.5) // 여기가 Z 시프팅의 핵심
    );

    vec3 local_pos = position;

    // ---- 4. 주변 샘플링 인덱스 세팅 ----

    int i_res = int(u_row_stride);
    int i_lod = int(u_lod_step);
    bool is_edge_x = (ix_lod == 0 || ix_lod == i_lod_res - 1);
    bool is_edge_y = (iy_lod == 0 || iy_lod == i_lod_res - 1);

    int iy_n = clamp(iy + i_lod, 0, i_res - 1);
    int iy_s = clamp(iy - i_lod, 0, i_res - 1);
    int ix_e = clamp(ix + i_lod, 0, i_res - 1);
    int ix_w = clamp(ix - i_lod, 0, i_res - 1);

    float h_n = float(texelFetch(u_heightmap, ivec2(ix, iy_n), 0).r) * u_scale;
    float h_s = float(texelFetch(u_heightmap, ivec2(ix, iy_s), 0).r) * u_scale;
    float h_e = float(texelFetch(u_heightmap, ivec2(ix_e, iy), 0).r) * u_scale;
    float h_w = float(texelFetch(u_heightmap, ivec2(ix_w, iy), 0).r) * u_scale;

    // ---- 5. 스커트(Skirt) 하단 경계 계산 ----

    float target_bottom;
    if (is_edge_x || is_edge_y) {
      target_bottom = surface_z - 200.0;
    } else {
      target_bottom = min(min(h_n, h_s), min(h_e, h_w)) - visual_scale;
    }

    float cell_bottom_z = surface_z - visual_scale;
    v_is_skirt = 0.0;

    if (local_pos.z < 0.0 && target_bottom < cell_bottom_z) {
      float extra_drop = (cell_bottom_z - target_bottom) / visual_scale;
      local_pos.z -= extra_drop;
      if (is_edge_x || is_edge_y) v_is_skirt = 1.0;
    }

    v_is_side_face = (position.z < 0.45) ? 1.0 : 0.0;

    vec3 scaled_local_pos = local_pos * visual_scale * 1.002;
    vec3 transformed = scaled_local_pos + instance_pos;

    vec4 world_pos = modelMatrix * vec4(transformed, 1.0);
    vec4 mvPosition = viewMatrix * world_pos;

    gl_Position = projectionMatrix * mvPosition;

    v_view_pos = mvPosition.xyz;
    v_view_dist = -mvPosition.z;
    v_surface_z = surface_z;
    v_world_xy = world_pos.xy;

    #include <fog_vertex>
  }
`;

// fragment shader
export function create_frag(): string {
  const ch_uniforms_decl = [
    ...GROUND_CH_KEYS.map((ch_key) => `uniform sampler2D ${meta.gen.U_CH(ch_key)};`),
    `uniform sampler2D u_edit_mask;`,
  ].join("\n  ");

  const ch_sampling_logic = GROUND_CH_KEYS
    .map((ch_key) => `float ${meta.gen.CH(ch_key)} = floor(texture(${meta.gen.U_CH(ch_key)}, v_uv).r * 255.0 + 0.5);`)
    .join("\n    ");
  const color_meta = meta.get_color_meta();
  const { color_uniforms_decl, color_mapping_funcs } = meta.build_color_mappings(color_meta);

  const base_land_uniform = meta.gen.U_COLOR(GroundChannelKey.LAND, "BARE");

  const ch_blending_logic = GROUND_CH_KEYS
    .map((ch_key) => {
      const ch_var_name: string = meta.gen.CH(ch_key);
      return `if (${ch_var_name} > 0.5) final_color = ${meta.gen.FUNC_COLOR(ch_key)}(${ch_var_name});`;
    })
    .join("");

  return /* glsl */ `
  precision highp float;
  #include <fog_pars_fragment>

  ${ch_uniforms_decl}
  ${color_uniforms_decl}

  uniform float u_border_opacity;

  in vec2 v_uv;
  in vec2 v_box_uv;
  in float v_is_side_face;
  in float v_is_skirt;
  in float v_view_dist;
  in vec3 v_view_pos;
  in float v_surface_z;
  in vec2 v_world_xy;

  out vec4 out_color;

  ${color_mapping_funcs}

  float getDynamicGrid(vec2 uv, float target_width) {
    vec2 fw = fwidth(uv);
    vec2 dist = abs(fract(uv - 0.5) - 0.5);
    vec2 norm_dist = dist / fw;
    float line_x = 1.0 - smoothstep(target_width - 0.5, target_width + 0.5, norm_dist.x);
    float line_y = 1.0 - smoothstep(target_width - 0.5, target_width + 0.5, norm_dist.y);
    return max(line_x, line_y);
  }

  float get_luminance(vec3 color) {
    return dot(color, vec3(0.299, 0.587, 0.114));
  }

  float saturate(float x) {
    return clamp(x, 0.0, 1.0);
  }

  vec3 rotate_tint(vec3 base, vec3 tint, float strength) {
    return mix(base, base * tint, strength);
  }

  float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
  }

  void main() {
    float edit_mask = texture(u_edit_mask, v_uv).r;
    if (edit_mask > 0.5) discard;

    ${ch_sampling_logic}
    bool is_water = ${meta.gen.CH(GroundChannelKey.WATER)} > 0.5;
    bool is_land = ${meta.gen.CH(GroundChannelKey.LAND)} > 0.5;
    bool is_structure = ${meta.gen.CH(GroundChannelKey.STRUCTURE)} > 0.5;
    bool is_transport = ${meta.gen.CH(GroundChannelKey.TRANSPORT)} > 0.5;
    vec3 final_color = ${base_land_uniform};
    ${ch_blending_logic}

    vec3 fdx = dFdx(v_view_pos);
    vec3 fdy = dFdy(v_view_pos);
    vec3 normal = normalize(cross(fdx, fdy));
    vec3 view_dir = normalize(-v_view_pos);

    vec3 light_dir = normalize(vec3(0.4, 0.35, 1.0));
    float diff = max(dot(normal, light_dir), 0.0);

    float light_intensity = 0.82 + (0.18 * diff);
    final_color *= light_intensity;

    // ---- channel material profiles ----
    //
    // goal:
    // - land / grass: softer and warmer
    // - roads: flatter / cleaner
    // - structures: denser / more matte
    if (v_is_skirt < 0.5 && v_is_side_face < 0.5) {
      if (is_land && !is_water) {
        final_color *= vec3(1.008, 1.006, 1.000);
      }

      if (is_transport) {
        final_color *= vec3(0.998, 0.998, 1.002);
      }

      if (is_structure) {
        final_color *= vec3(0.992, 0.990, 0.988);
      }
    }

    // ---- subtle land height tone variation ----
    //
    // goal:
    // - make terrain feel less flat / printed
    // - keep it extremely soft, toy-like
    if (!is_water && v_is_skirt < 0.5 && v_is_side_face < 0.5) {
      float h_tone = 0.5 + 0.5 * sin(v_surface_z * 0.035);
      vec3 hi_tint = vec3(1.015, 1.012, 1.000);
      vec3 lo_tint = vec3(0.992, 0.992, 0.998);
      final_color *= mix(lo_tint, hi_tint, h_tone);

      // subtle material grain / roughness-like variation
      float grain_a = hash21(floor(v_world_xy * 0.35));
      float grain_b = hash21(floor(v_world_xy * 0.70 + vec2(11.0, 7.0)));
      float grain = mix(grain_a, grain_b, 0.5);
      float rough_lo = 0.985;
      float rough_hi = 1.015;

      if (is_transport) {
        rough_lo = 0.994;
        rough_hi = 1.004;
      } else if (is_structure) {
        rough_lo = 0.989;
        rough_hi = 1.006;
      } else if (is_land) {
        rough_lo = 0.982;
        rough_hi = 1.018;
      }

      float rough_tone = mix(rough_lo, rough_hi, grain);
      final_color *= vec3(rough_tone, rough_tone, rough_tone * 0.998);
    }

    // ---- directional side tint ----
    //
    // goal:
    // - break the uniform block-wall look
    // - keep side faces warm and miniature-like
    if (v_is_side_face > 0.5 && v_is_skirt < 0.5) {
      vec3 side_n = normalize(vec3(normal.x, normal.y, 0.0) + vec3(0.0001, 0.0001, 0.0));
      float eastness = saturate(side_n.x * 0.5 + 0.5);
      float northness = saturate(side_n.y * 0.5 + 0.5);

      vec3 east_tint = vec3(1.02, 0.99, 0.96);
      vec3 west_tint = vec3(0.96, 0.94, 0.92);
      vec3 north_tint = vec3(0.98, 1.00, 0.99);
      vec3 south_tint = vec3(1.01, 0.97, 0.94);

      vec3 ew_tint = mix(west_tint, east_tint, eastness);
      vec3 ns_tint = mix(south_tint, north_tint, northness);
      vec3 side_tint = mix(ew_tint, ns_tint, 0.35);

      float side_strength = 0.18;
      if (is_transport) {
        side_strength = 0.10;
      } else if (is_structure) {
        side_strength = 0.14;
      } else if (is_land && !is_water) {
        side_strength = 0.22;
      }

      final_color = rotate_tint(final_color, side_tint, side_strength);
    }

    if (v_is_side_face > 0.5) {
      vec3 side_mul = vec3(0.95, 0.92, 0.88);

      if (is_transport) {
        side_mul = vec3(0.97, 0.96, 0.95);
      } else if (is_structure) {
        side_mul = vec3(0.94, 0.92, 0.90);
      } else if (is_land && !is_water) {
        side_mul = vec3(0.94, 0.91, 0.87);
      }

      final_color *= side_mul;
    }

    if (v_is_skirt > 0.5) {
      final_color *= vec3(0.90, 0.87, 0.83);
    }

    if (v_is_skirt < 0.5 && v_is_side_face < 0.5) {
      final_color *= vec3(1.02, 1.01, 0.99);
    }

    // ---- water material lift ----
    //
    // goal:
    // - slightly cleaner / brighter than land
    // - less clay-like side tint
    // - softer top-face highlight
    if (is_water) {
      final_color *= vec3(0.99, 1.04, 1.07);

      if (v_is_side_face > 0.5) {
        final_color *= vec3(1.02, 1.04, 1.06);
      }

      if (v_is_skirt < 0.5 && v_is_side_face < 0.5) {
        final_color = mix(final_color, vec3(0.88, 0.99, 0.98), 0.14);

        // subtle toy-water fresnel lift
        float fresnel = pow(1.0 - saturate(dot(normal, view_dir)), 3.0);
        final_color = mix(final_color, vec3(0.96, 1.0, 0.99), fresnel * 0.14);

        // tiny specular-ish glint
        vec3 half_dir = normalize(light_dir + view_dir);
        float spec = pow(saturate(dot(normal, half_dir)), 24.0);
        final_color = mix(final_color, vec3(1.0, 1.0, 0.99), spec * 0.05);

        // subtle flow-like shimmer
        float flow_phase =
          sin(v_world_xy.x * 0.045 + v_world_xy.y * 0.020 + v_view_dist * 0.015) *
          sin(v_world_xy.y * 0.055 - v_world_xy.x * 0.018 + v_view_dist * 0.010);
        float shimmer = 0.5 + 0.5 * flow_phase;
        final_color = mix(final_color, vec3(0.95, 1.0, 0.99), shimmer * 0.06);
      }
    }

    if (v_is_skirt < 0.5 && v_is_side_face < 0.5) {
      float dist_factor = smoothstep(0.0, 480.0, v_view_dist);
      float dist_fade = 1.0 - dist_factor;
      float current_width = mix(1.25, 0.4, dist_factor);
      float opacity_fade = dist_fade * dist_fade;
      float line_alpha = getDynamicGrid(v_box_uv, current_width);

      if (is_water) {
        line_alpha *= 0.08;
      }

      if (line_alpha > 0.01) {
        float grid_strength = line_alpha * u_border_opacity * opacity_fade;
        if (grid_strength > 0.001) {
          float lum = get_luminance(final_color);
          vec3 adaptive_color = mix(vec3(1.0), vec3(0.0), smoothstep(0.3, 0.7, lum));
          final_color = mix(final_color, adaptive_color, grid_strength);
        }
      }
    }

    // ---- distance pastel lift ----
    //
    // goal:
    // - soften far terrain into a tiny-diorama feel
    // - not true bloom, just a pastel atmospheric lift
    float pastel_factor = smoothstep(180.0, 720.0, v_view_dist);
    vec3 pastel_tint = vec3(1.02, 1.015, 1.01);

    if (is_water) {
      pastel_tint = vec3(1.01, 1.02, 1.025);
    }

    final_color = mix(final_color, final_color * pastel_tint, pastel_factor * 0.18);

    out_color = vec4(final_color, 1.0);

    #ifdef USE_FOG
      float fog_factor = 0.0;
      #ifdef FOG_EXP2
        fog_factor = 1.0 - exp( - fogDensity * fogDensity * vFogDepth * vFogDepth );
      #else
        fog_factor = smoothstep( fogNear, fogFar, vFogDepth );
      #endif

      if (is_water) {
        fog_factor *= 0.88;
      }

      out_color.rgb = mix( out_color.rgb, fogColor, fog_factor );
    #endif
  }
  `;
}
