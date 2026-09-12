// @pxl/web/modules/world/billboard/field.ts

/**
 * Every billboard of one tier, as one instanced draw.
 *
 * What this replaces mounted a React subtree per badge — a component, an
 * AnchorNode with its own Group and portal, a Billboard, a SpriteBillboard —
 * and then, for a while, a pool of `THREE.Sprite`s written by index: one
 * material, one texture window and one draw call per badge, plus a loop
 * every frame that rescaled and repositioned each of them on the CPU. On a
 * phone the badges were most of the frame's draw calls (2026-09-13).
 *
 * A badge is a position, a size and a rectangle of the sheet. Those are
 * per-instance attributes on one quad now, and everything the per-frame
 * loop did — the fit to screen pixels, the distance fade, the anchor
 * pinning, the occlusion twin — is the vertex shader's, from the camera
 * and a few uniforms. The CPU writes the attributes when a placement
 * commits and nothing between commits. One draw whatever the count, and
 * the sheet is bound once rather than cloned into a window per slot.
 *
 * It is also the layer that has to survive the move off three.js. Lanes
 * written by index and one instanced draw are what WebGPU wants; a React
 * element per badge had no counterpart in linear memory at all.
 *
 * React keeps exactly one job here: mounting `group` once.
 */

import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { fade_end_u, fade_start_u } from "@/render/lod.ts";

import type { BillboardAtlas } from "./atlas.ts";
import { OCCLUSION_FRAG_FUNCS, OCCLUSION_FRAG_MAIN, occlusion_uniforms } from "./anchor_occlusion.ts";

const INIT_CAP: u32 = 256;

// Per instance: the anchor in the group's frame, the logical size in
// pixels, the sheet rectangle, and where the anchor falls down the sprite
// (0 top, 1 bottom). The corner comes from the unit quad's `position`.
//
// The fit is distance_fade.ts's: a fully visible badge is drawn at one
// screen pixel per logical pixel, so its world size is logical px times
// distance over the projection's pixels-per-unit-at-one-unit; past the
// fade's start it shrinks and dims by a smoothstep and is gone at the end.
// The centre sits above the anchor by the anchor's own offset so the dot
// stays pinned while the pill rescales, and the quad extends from that
// centre along the camera's right and up. The occlusion varyings are
// anchor_occlusion.ts's, computed here from the same quantities the sprite
// patch read off modelMatrix.
const VERT = /* glsl */ `attribute vec3 i_anchor;
attribute vec2 i_size;
attribute vec4 i_uv;
attribute float i_anchor_frac;
uniform float u_px_k;
uniform float u_fade_start;
uniform float u_fade_end;
uniform mat4 u_depth_view_projection;
uniform float u_depth_bias;
uniform float u_depth_cell;
uniform float u_depth_bias_cells;
uniform float u_depth_lift;
varying vec2 v_uv;
varying float v_opacity;
varying vec4 v_anchor_clip;
varying vec4 v_rise_clip;
varying vec4 v_upright_clip;

void main() {
	vec3 anchor_world = (modelMatrix * vec4(i_anchor, 1.0)).xyz;
	vec3 anchor_view = (viewMatrix * vec4(anchor_world, 1.0)).xyz;
	float dist = length(anchor_view);
	float far = 1.0 - smoothstep(u_fade_start, u_fade_end, dist);
	vec2 size = i_size * dist / u_px_k * far;
	v_opacity = far;

	float drop = size.y * (i_anchor_frac - 0.5);
	vec3 centre_view = (viewMatrix * vec4(anchor_world + vec3(0.0, 0.0, drop), 1.0)).xyz;
	vec2 corner = position.xy * size;
	gl_Position = projectionMatrix * vec4(centre_view + vec3(corner, 0.0), 1.0);

	v_uv = vec2(mix(i_uv.x, i_uv.z, uv.x), mix(i_uv.w, i_uv.y, uv.y));

	// The visibility question, asked up the badge from just above the anchor.
	v_anchor_clip = u_depth_view_projection * vec4(anchor_world + vec3(0.0, 0.0, u_depth_lift), 1.0);
	v_rise_clip = u_depth_view_projection * vec4(0.0, 0.0, size.y, 0.0);

	// The upright twin, for depth: the corner's Y along world up rather than
	// the camera's, nudged toward the camera by the same tolerance.
	vec3 world_up_view = normalize((viewMatrix * vec4(0.0, 0.0, 1.0, 0.0)).xyz);
	float anchor_dist = max(0.0, -centre_view.z);
	float depth_push = u_depth_bias + anchor_dist * u_depth_cell * u_depth_bias_cells;
	v_upright_clip = projectionMatrix * vec4(centre_view + vec3(corner.x, 0.0, depth_push) + world_up_view * corner.y, 1.0);
}`;

const FRAG = /* glsl */ `uniform sampler2D t_sheet;
varying vec2 v_uv;
varying float v_opacity;
${OCCLUSION_FRAG_FUNCS}
void main() {
${OCCLUSION_FRAG_MAIN}
	vec4 texel = texture2D(t_sheet, v_uv);
	gl_FragColor = vec4(texel.rgb, texel.a * v_opacity);
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`;

export class BillboardField {
  readonly group = new THREE.Group();
  /** Live this frame. */
  count: u32 = 0;

  private cap: u32 = 0;
  private readonly geometry = new THREE.InstancedBufferGeometry();
  private readonly material: THREE.ShaderMaterial;
  private readonly mesh: THREE.Mesh;
  private anchor!: THREE.InstancedBufferAttribute;
  private size!: THREE.InstancedBufferAttribute;
  private rect!: THREE.InstancedBufferAttribute;
  private anchor_frac!: THREE.InstancedBufferAttribute;
  private sheet: THREE.Texture | null = null;

  constructor(render_order: f64) {
    this.group.name = "billboard-field";
    this.group.frustumCulled = false;

    const quad = new THREE.PlaneGeometry(1, 1);
    this.geometry.setIndex(quad.getIndex());
    this.geometry.setAttribute("position", quad.getAttribute("position"));
    this.geometry.setAttribute("uv", quad.getAttribute("uv"));
    this.geometry.instanceCount = 0;
    this.reserve(INIT_CAP);

    this.material = new THREE.ShaderMaterial({
      vertexShader: VERT,
      fragmentShader: FRAG,
      uniforms: {
        t_sheet: { value: null },
        u_px_k: { value: 1 },
        u_fade_start: { value: fade_start_u() },
        u_fade_end: { value: fade_end_u() },
        ...occlusion_uniforms(),
      },
      transparent: true,
      depthTest: true,
      // Depth is written by the occlusion shader's upright twin through
      // gl_FragDepth for the test's sake; nothing behind a badge should be
      // hidden by it.
      depthWrite: false,
    });

    this.mesh = new THREE.Mesh(this.geometry, this.material);
    this.mesh.renderOrder = render_order;
    // Instances carry their own positions; the mesh has no bounds to cull by.
    this.mesh.frustumCulled = false;
    this.group.add(this.mesh);
  }

  /** Opens a placement. */
  begin(): void {
    this.count = 0;
  }

  /**
   * Writes one badge into the next instance, growing the buffers if this is
   * the most that have ever been live at once. The badge is `atlas_slot` of
   * `atlas`; its rectangle is read now, since a repack moves rectangles and
   * not slots.
   */
  push(x: f64, y: f64, z: f64, atlas: BillboardAtlas, atlas_slot: u32, logical_w: f64, logical_h: f64, center_y: f64): void {
    const i = this.count++;
    if (i >= this.cap) this.reserve(this.cap * 2);

    this.anchor.setXYZ(i, x, y, z);
    this.size.setXY(i, logical_w, logical_h);
    this.rect.setXYZW(i, atlas.u0(atlas_slot), atlas.v0(atlas_slot), atlas.u1(atlas_slot), atlas.v1(atlas_slot));
    this.anchor_frac.setX(i, logical_h > 0 ? center_y / logical_h : 1);

    // A grown sheet is a different texture over a different image, so
    // every badge samples the new one from the moment it exists — including
    // the ones written earlier in this pass.
    if (atlas.texture !== this.sheet) {
      this.sheet = atlas.texture;
      this.material.uniforms.t_sheet.value = this.sheet;
    }
  }

  /** Closes the placement: what was written goes to the GPU, and the draw
   * covers exactly that many. */
  end(): void {
    for (const attribute of [this.anchor, this.size, this.rect, this.anchor_frac]) {
      attribute.clearUpdateRanges();
      attribute.addUpdateRange(0, this.count * attribute.itemSize);
      attribute.needsUpdate = true;
    }
    this.geometry.instanceCount = this.count;
  }

  /**
   * Per frame: the projection's pixels per unit at one unit of distance,
   * and the fade's live range. Everything per badge is the shader's.
   */
  update(camera: THREE.Camera, viewport_h: f64): void {
    const fov = (camera as THREE.PerspectiveCamera).fov;
    this.material.uniforms.u_px_k.value = viewport_h / (2 * Math.tan((fov * Math.PI) / 360));
    this.material.uniforms.u_fade_start.value = fade_start_u();
    this.material.uniforms.u_fade_end.value = fade_end_u();
  }

  /** Dev: what the buffers hold and show, for the ledger's own report. */
  stats(): { slots: u32; live: u32; visible: u32; mapped: u32; mounted: boolean } {
    // Mounted means reachable from a scene, not merely having a parent — a
    // frame group that was replaced would still be a parent.
    let root: THREE.Object3D | null = this.group;
    while (root.parent) root = root.parent;
    return { slots: this.cap, live: this.count, visible: this.count, mapped: this.sheet ? this.count : 0, mounted: (root as THREE.Scene).isScene === true };
  }

  /** Releases every GPU resource this owns. The sheet is the atlas's. */
  dispose(): void {
    this.geometry.dispose();
    this.material.dispose();
    this.group.clear();
    this.count = 0;
  }

  /** Buffers for `cap` instances, copying what is live. */
  private reserve(cap: u32): void {
    const make = (item_size: u32, previous: THREE.InstancedBufferAttribute | undefined) => {
      const array = new Float32Array(cap * item_size);
      if (previous) array.set((previous.array as Float32Array).subarray(0, Math.min(previous.count, this.count) * item_size));
      const attribute = new THREE.InstancedBufferAttribute(array, item_size);
      attribute.setUsage(THREE.DynamicDrawUsage);
      return attribute;
    };
    this.anchor = make(3, this.anchor);
    this.size = make(2, this.size);
    this.rect = make(4, this.rect);
    this.anchor_frac = make(1, this.anchor_frac);
    this.geometry.setAttribute("i_anchor", this.anchor);
    this.geometry.setAttribute("i_size", this.size);
    this.geometry.setAttribute("i_uv", this.rect);
    this.geometry.setAttribute("i_anchor_frac", this.anchor_frac);
    this.cap = cap;
  }
}
