// @pxl/web/modules/world/billboard/anchor_occlusion.ts

/**
 * How a billboard meets the world's depth. Two questions, deliberately
 * separate.
 *
 * **Is the labelled point visible at all?** Asked once, at the anchor, and
 * answered for the whole sprite.
 *
 * Depth-testing a Sprite normally does not do this. A Sprite is shaded as a
 * quad but positioned as a point — three.js takes view Z from
 * `modelViewMatrix[3]` and offsets only XY — so every fragment carries the
 * anchor's depth while the depth test still runs per fragment. Each pixel
 * ends up asking whether the terrain behind *it* is nearer than the anchor,
 * and different pixels answer differently: what you get is a band of terrain
 * punched through the middle of a badge, not a badge behind a hill.
 *
 * So the question is asked once, at the anchor, and answered for the whole
 * sprite. The vertex shader projects the anchor — the point the badge's own
 * dot marks, not the middle of its pill — into the frame the depth copy was
 * taken in; the fragment shader samples
 * render/scene_depth.ts's copy there and discards everything if the world is
 * in front. All fragments read the same texels, so the result is
 * all-or-nothing by construction.
 *
 * Reprojecting rather than using the current frame's screen position is what
 * makes the copy's one-frame staleness invisible. Sampling last frame's
 * depth at this frame's pixel reads whatever has moved under that pixel
 * since, which while the camera turns is a different piece of world every
 * frame — badges vanishing mid-rotation with nothing in front of them.
 *
 * **Is something standing between the camera and the badge itself?** Left to
 * the GPU's own depth test, which is only usable here because the shader
 * writes the depth of an *upright twin* of the quad rather than the quad's
 * own. A billboard is shaded flat against the camera but is meant to read as
 * a label hanging above its anchor, and those two shapes have different
 * depths: the flat one puts every fragment at the anchor's distance, so a
 * terrace a little nearer than the anchor cuts a band through the middle of
 * a badge. The upright twin gives each fragment the depth of the point it
 * actually represents, so ground-level relief passes under a badge and a
 * tree at badge height still clips it.
 *
 * The alternative was a CPU raycast per badge. That is what the previous POI
 * implementation did and what this tier was rebuilt to stop doing: each miss
 * in the raycast volume cache (four chunks, sized for one hover ray) costs a
 * whole ChunkVolume composite, which is the same work the mesher does per
 * chunk.
 */

import * as THREE from "three";

import type { f64 } from "@xpute/core/kit/word.ts";
import { LOD_CELL_PER_DISTANCE } from "@/render/lod.ts";
import { SCENE_DEPTH } from "@/render/scene_depth.ts";

/**
 * How much nearer the world has to be before it counts as in front, in
 * world units (one cell).
 *
 * A badge sits a fraction of a cell above the ground it labels, so an exact
 * comparison flickers against its own anchor surface as the rasterised
 * depth of that ground shifts by a fraction of a unit under camera motion.
 *
 * World units, not depth-buffer units, because a depth-buffer tolerance is
 * a different world distance at every range: the value that stops a badge
 * fighting the ground under it up close is wide enough to see through a
 * building in the distance. That is what a previous version got wrong.
 * A cell is small against anything worth being hidden behind and large
 * against the clearance a badge floats at.
 */
const DEPTH_BIAS_U: f64 = 1;

/**
 * Extra tolerance in coarse cells, on top of the fixed one.
 *
 * Two things grow with distance and both are covered by one term. A depth
 * buffer spends almost all its precision near the camera, so the world
 * distance a stored value can be wrong by grows with the square of the
 * range — under a unit up close, several units once the view is pulled
 * back. And the terrain drawn at a range is the tier for that range: a
 * badge is grounded on the fine elevation, while a coarse box is shrunk
 * about its cell's centre, so on a slope its top can sit up to a cell above
 * the ground the badge stands on — and inside a blend band two tiers take
 * turns, so which surface the depth copy holds changes with the camera.
 * Both are proportional to distance, and the tier ladder says how: the
 * cell drawn at a range is at most LOD_CELL_PER_DISTANCE of that range.
 *
 * A cell and a half, so a badge buried by the coarse box of its own slope
 * is still shown with margin. At two thousand units that is about thirty
 * units, which at that range is a few pixels of building a badge could be
 * shown through — nothing anyone can point at. A fixed fraction here was
 * 0.5%, under one cell just past a threshold, which is exactly the margin
 * a coarse box on a slope eats — badges on the camera-facing side of every
 * ridge blinked on and off while the flat ones held still (2026-09-12).
 */
const DEPTH_BIAS_CELLS: f64 = 1.5;

/**
 * Where the visibility question is asked: at several points up the badge's
 * own height, from just above the anchor to the top of the pill, and the
 * badge is hidden only when most of them are behind the world.
 *
 * Asked at one point — the anchor, which is the ground point itself — the
 * question was "is the ground in front of the ground", and on any slope
 * facing the camera the pixel under the anchor shows the next cell up, a
 * cell nearer, or at a coarse tier a box a cell or two tall; whether that
 * cleared the tolerance flipped with every fraction of a unit the camera
 * moved, and zooming walked badges on the smallest rises through the tiers
 * so they came and went with each (2026-09-12). A single texel of a
 * half-resolution copy is also just one texel.
 *
 * Coverage is what a label really means by hidden: a ridge that clips the
 * dot leaves the name readable, a hill in front of the pill does not. So a
 * badge with a third of its height behind the world is drawn, and one with
 * more than half is not — and one texel cannot flip that on its own.
 */
const DEPTH_SAMPLES = 5;
/** Hidden once this many of DEPTH_SAMPLES are behind the world. */
const DEPTH_HIDE_AT = 3;
/** The lowest sample sits this far above the anchor, so the dot's own cell
 * is never the one asked about. */
const DEPTH_LIFT_U: f64 = 2;

// modelMatrix's translation column is the sprite's world position, which is
// its *centre*, not its anchor: use_distance_fade moves that centre every
// frame so the anchor point stays pinned at base_z as the sprite rescales.
// u_anchor_drop is how far down from the centre the anchor currently sits,
// written by the same hook that does the moving. Measuring from the centre
// instead asks about a point in the middle of the pill, which is a
// different place in the world and a different distance from the camera.
//
// A plain world-Z subtraction is enough because the only thing that ever
// rotates between a billboard and the world is azimuth, about Z (world.tsx's
// world_frame_ref), and rotating about Z leaves Z alone.
const VERT_DECL = /* glsl */ `varying vec4 v_anchor_clip;
varying vec4 v_rise_clip;
varying vec4 v_upright_clip;
uniform mat4 u_depth_view_projection;
uniform float u_depth_bias;
uniform float u_depth_cell;
uniform float u_depth_bias_cells;
uniform float u_anchor_drop;
uniform float u_depth_lift;
void main() {
	vec3 anchor_world = modelMatrix[3].xyz - vec3(0.0, 0.0, u_anchor_drop - u_depth_lift);
	v_anchor_clip = u_depth_view_projection * vec4(anchor_world, 1.0);
	// The badge's height in world units is the sprite's own Y scale; its
	// samples climb from the anchor by fractions of it, along world up.
	v_rise_clip = u_depth_view_projection * vec4(0.0, 0.0, length(modelMatrix[1].xyz), 0.0);`;

// The same quad, positioned as if it stood upright in the world instead of
// lying flat against the camera — used for depth only, never for shape.
//
// modelViewMatrix[3] is the anchor in view space, and `rotatedPosition` is
// the corner offset three.js is about to apply there in view X and Y. Y is
// the camera's own up, which is what makes every corner share the anchor's
// depth; sending it along the world's up instead gives the corner the depth
// of the point it is actually meant to represent — a label hanging in the
// air above its anchor, not a card lying at the anchor's distance.
//
// X stays the camera's right. For a camera that never rolls that is already
// horizontal, which is the axis an upright card would use anyway.
//
// The same tolerance the anchor test uses nudges it toward the camera (view
// +Z), so the bottom edge — which really does sit at the anchor, half a cell
// over the ground — does not fight that ground.
//
// Including the distance-relative half, which is the whole point: this depth
// goes to the GPU's own test, and that test has no tolerance of its own. A
// fixed nudge left the badge losing outright once the view pulled back far
// enough for depth quantisation to exceed it — the entire sprite failing
// depth with nothing in front of it. Both paths ask the same question, so
// both get the same tolerance.
const VERT_UPRIGHT = /* glsl */ `mvPosition.xy += rotatedPosition;
	vec3 world_up_view = normalize((viewMatrix * vec4(0.0, 0.0, 1.0, 0.0)).xyz);
	float anchor_dist = max(0.0, -modelViewMatrix[3].z);
	float depth_push = u_depth_bias + anchor_dist * u_depth_cell * u_depth_bias_cells;
	v_upright_clip = projectionMatrix * vec4(
		modelViewMatrix[3].xyz + vec3(rotatedPosition.x, 0.0, depth_push) + world_up_view * rotatedPosition.y,
		1.0
	);`;

/** The fragment side, in two parts the instanced field shares: the
 * declarations and functions, and the body that runs first in main. */
export const OCCLUSION_FRAG_FUNCS = /* glsl */ `varying vec4 v_anchor_clip;
varying vec4 v_rise_clip;
varying vec4 v_upright_clip;
uniform sampler2D t_scene_depth;
uniform vec2 u_depth_near_far;
uniform float u_depth_ready;
uniform float u_depth_bias;
uniform float u_depth_cell;
uniform float u_depth_bias_cells;

// Distance from the camera, in world units, for a depth-buffer value.
float view_distance(float depth01) {
	float ndc_z = depth01 * 2.0 - 1.0;
	float near = u_depth_near_far.x, far = u_depth_near_far.y;
	return (2.0 * near * far) / (far + near - ndc_z * (far - near));
}

// Whether the world is in front of a point of the badge, given in the
// depth copy's clip space. A point off the edge of the copy has no depth to
// read; it is not necessarily off *this* frame's screen though — the copy
// is the previous frame's — so it counts as visible rather than hidden.
bool behind_world(vec4 clip) {
	if (clip.w <= 0.0) return false;
	vec3 ndc = clip.xyz / clip.w;
	vec2 depth_uv = ndc.xy * 0.5 + 0.5;
	if (any(lessThan(depth_uv, vec2(0.0))) || any(greaterThan(depth_uv, vec2(1.0)))) return false;
	float world = view_distance(texture2D(t_scene_depth, depth_uv).x);
	float point = view_distance(ndc.z * 0.5 + 0.5);
	return world < point - u_depth_bias - point * u_depth_cell * u_depth_bias_cells;
}
`;

export const OCCLUSION_FRAG_MAIN = /* glsl */ `	if (u_depth_ready > 0.5) {
		int hidden = 0;
		for (int i = 0; i < ${DEPTH_SAMPLES}; i++) {
			if (behind_world(v_anchor_clip + v_rise_clip * (float(i) / float(${DEPTH_SAMPLES - 1})))) hidden++;
		}
		if (hidden >= ${DEPTH_HIDE_AT}) discard;
	}
	// Depth of the upright twin, so the GPU's own test can clip this badge
	// against anything genuinely standing between it and the camera. Written
	// unconditionally: leaving it to gl_FragCoord.z would put the whole quad
	// back at the anchor's distance, which is the slicing this file exists to
	// avoid.
	gl_FragDepth = clamp(v_upright_clip.z / v_upright_clip.w * 0.5 + 0.5, 0.0, 1.0);`;

/** The uniforms both the sprite patch and the instanced field bind. The
 * objects come from SCENE_DEPTH and are shared across every billboard, so
 * publishing a new copy each frame is one assignment there. */
export function occlusion_uniforms(): Record<string, THREE.IUniform> {
  return {
    t_scene_depth: SCENE_DEPTH.texture,
    u_depth_ready: SCENE_DEPTH.ready,
    u_depth_view_projection: SCENE_DEPTH.view_projection,
    u_depth_near_far: SCENE_DEPTH.near_far,
    u_depth_bias: { value: DEPTH_BIAS_U },
    u_depth_cell: LOD_CELL_PER_DISTANCE,
    u_depth_bias_cells: { value: DEPTH_BIAS_CELLS },
    u_depth_lift: { value: DEPTH_LIFT_U },
  };
}

/**
 * Patches one SpriteMaterial in place. The uniform objects come from
 * SCENE_DEPTH and are shared across every billboard, so publishing a new
 * copy each frame is one assignment there rather than a walk over materials
 * here.
 */
export function apply_anchor_occlusion(material: THREE.SpriteMaterial): THREE.SpriteMaterial {
  const anchor_drop = { value: 0 };
  material.userData.anchor_drop = anchor_drop;
  material.onBeforeCompile = (shader) => {
    shader.uniforms.u_anchor_drop = anchor_drop;
    Object.assign(shader.uniforms, occlusion_uniforms());
    shader.vertexShader = shader.vertexShader
      .replace("void main() {", VERT_DECL)
      .replace("mvPosition.xy += rotatedPosition;", VERT_UPRIGHT);
    shader.fragmentShader = shader.fragmentShader.replace("void main() {", `${OCCLUSION_FRAG_FUNCS}\nvoid main() {\n${OCCLUSION_FRAG_MAIN}`);
  };
  // Distinct from an unpatched SpriteMaterial's, so three does not hand this
  // material a cached program compiled without the injection.
  material.customProgramCacheKey = () => "billboard_anchor_occlusion";
  return material;
}
