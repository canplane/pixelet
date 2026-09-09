// @pxl/web/render/scene_depth.ts

/**
 * The world's depth buffer, kept readable by ordinary scene materials.
 *
 * A material drawn inside the scene pass cannot read the depth texture that
 * same pass is writing. This publishes a copy taken after the scene render,
 * so anything drawn in the *next* frame's scene pass can ask "is this world
 * point visible". One frame stale, which is invisible for a visibility
 * answer and is the whole price of not rendering the world a second time to
 * find out.
 *
 * Deliberately not a by-product of any effect. N8AO needs depth today and is
 * expected to go once shading is baked into the textures, so this pass asks
 * for depth itself (`needsDepthTexture`) and stays in the chain whatever the
 * post stack is doing. It is a renderer service, not a look.
 *
 * The copy is half-resolution and NearestFilter, because every consumer so
 * far reads a single point (billboard/sprite_billboard.tsx asks about one
 * anchor). A filtered depth is a blend of two surfaces and is the depth of
 * neither.
 */

import * as THREE from "three";
import { Pass } from "postprocessing";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

/** Half the drawing buffer per axis — a quarter of the fragments, still far
 * finer than the one point a consumer reads. */
const DEPTH_SCALE: f64 = 0.5;

/**
 * Shared by every consumer material's uniforms, mutated in place by the pass
 * below. One object rather than a per-material copy and a per-frame walk to
 * update them all: three reads whatever the uniform object holds at draw
 * time, so publishing is a single assignment.
 *
 * `ready` is 0 until the first copy exists. Sampling an unbound sampler
 * yields an implementation-defined value, and a consumer that reads it as
 * "something is in front of you" would blink on the first frame.
 */
export const SCENE_DEPTH = {
  texture: { value: null as THREE.Texture | null },
  ready: { value: 0 },
  /**
   * The view-projection the copy was rendered with — the *previous* frame's.
   *
   * A consumer must project its world point with this, not with the camera
   * it is currently being drawn under. Sampling a stale depth buffer at a
   * fresh screen position reads whatever has since moved under that pixel,
   * which during a camera rotation is a different piece of world every
   * frame. For a world-static point this reprojection is exact, and the
   * staleness stops being observable at all.
   */
  view_projection: { value: new THREE.Matrix4() },
  /**
   * The camera's near and far, so a consumer can undo the depth buffer's
   * non-linearity and compare in world units.
   *
   * Comparing raw depth values needs a tolerance, and a tolerance in
   * non-linear depth means a different world distance at every range — tight
   * enough to be correct up close is loose enough to hide a building far
   * away, and vice versa. A world-unit tolerance is the same everywhere.
   */
  near_far: { value: new THREE.Vector2(0.1, 1000) },
};

const COPY_VERT = /* glsl */ `
varying vec2 v_uv;
void main() {
  v_uv = position.xy * 0.5 + 0.5;
  gl_Position = vec4(position.xy, 0.0, 1.0);
}`;

// The raw non-linear depth, passed through unchanged. A consumer compares it
// against its own clip-space z, so nothing has to agree about near/far —
// only about the projection, which is the same camera either way.
const COPY_FRAG = /* glsl */ `
uniform sampler2D t_depth;
varying vec2 v_uv;
void main() {
  gl_FragColor = vec4(texture2D(t_depth, v_uv).x, 0.0, 0.0, 1.0);
}`;

export class SceneDepthPass extends Pass {
  private readonly target: THREE.WebGLRenderTarget;
  private readonly copy_material: THREE.ShaderMaterial;
  /** The composer sets this; the base class exposes only a setter, and the
   * copy has to be stamped with the camera it was taken under. */
  private scene_camera: THREE.Camera | null = null;

  constructor() {
    super("SceneDepthPass");
    this.needsDepthTexture = true;
    // This pass produces a side channel, never the image the chain is
    // building. Without this it would consume its turn in the ping-pong and
    // hand the next pass its own depth copy as the colour input.
    this.needsSwap = false;
    this.renderToScreen = false;

    this.target = new THREE.WebGLRenderTarget(1, 1, {
      minFilter: THREE.NearestFilter,
      magFilter: THREE.NearestFilter,
      depthBuffer: false,
      stencilBuffer: false,
      type: THREE.FloatType,
    });
    this.target.texture.name = "scene_depth";
    this.target.texture.generateMipmaps = false;

    this.copy_material = new THREE.ShaderMaterial({
      uniforms: { t_depth: { value: null } },
      vertexShader: COPY_VERT,
      fragmentShader: COPY_FRAG,
      depthTest: false,
      depthWrite: false,
    });
    this.fullscreenMaterial = this.copy_material;
  }

  override set mainCamera(camera: THREE.Camera) {
    super.mainCamera = camera;
    this.scene_camera = camera;
  }

  override setDepthTexture(depth_texture: THREE.Texture, _depth_packing?: number): void {
    this.copy_material.uniforms.t_depth.value = depth_texture;
  }

  override setSize(width: u32, height: u32): void {
    this.target.setSize(
      Math.max(1, Math.round(width * DEPTH_SCALE)),
      Math.max(1, Math.round(height * DEPTH_SCALE)),
    );
  }

  override render(renderer: THREE.WebGLRenderer): void {
    if (!this.copy_material.uniforms.t_depth.value) return;
    renderer.setRenderTarget(this.target);
    renderer.render(this.scene, this.camera);
    SCENE_DEPTH.texture.value = this.target.texture;
    const camera = this.scene_camera;
    if (camera) {
      SCENE_DEPTH.view_projection.value.multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
      const perspective = camera as THREE.PerspectiveCamera;
      if (perspective.isPerspectiveCamera) SCENE_DEPTH.near_far.value.set(perspective.near, perspective.far);
      SCENE_DEPTH.ready.value = 1;
    }
  }

  override dispose(): void {
    if (SCENE_DEPTH.texture.value === this.target.texture) {
      SCENE_DEPTH.texture.value = null;
      SCENE_DEPTH.ready.value = 0;
    }
    this.target.dispose();
    this.copy_material.dispose();
    super.dispose();
  }
}
