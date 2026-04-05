// @pxl/web/modules/map/render/tick-controller.ts

import * as THREE from "three";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";
import { clamp, wrap } from "@xpute/spec/lib/kit/math.ts";

import { fvec, type IVec, ivec, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";

import { useCameraStore } from "@/modules/camera/store.ts";
import { camera_cfg } from "@/modules/camera/config.ts";
import { render_cfg } from "@/modules/world/render/config.ts";

const { MIN_DIST, MAX_DIST, MIN_POLAR, MAX_POLAR } = camera_cfg.view;
const { CAM_DAMP, ROT_DAMP, POS_DAMP } = camera_cfg.smooth;

const { FAR_CLIP } = render_cfg;

/** Native Z-up direction for the web world renderer. */
export const CAMERA_UP: [f64, f64, f64] = [0, 0, 1];

/**
 * Render tick controller.
 *
 * ROLE:
 * - owns per-frame camera and frame transform updates
 * - keeps render math out of React hooks/components
 * - lets R3F act only as a tick entry surface
 *
 * NOTES:
 * - camera pose is derived from CameraStore view intent
 * - world_frame_ref carries azimuth smoothing
 * - local_world_frame_ref carries floating-origin translation smoothing
 */
export class RenderTickController {
  private did_init_camera = false;
  private did_init_rot = false;
  private did_init_pos = false;

  private dist: f64 = 0;
  private polar: f64 = 0;

  private prev_map_origin: IVec = 0n;

  init_camera(camera: THREE.PerspectiveCamera): void {
    camera.up.set(...CAMERA_UP);
    camera.far = FAR_CLIP;
    camera.updateProjectionMatrix();
  }

  tick(props: {
    camera: THREE.PerspectiveCamera;
    world_frame: THREE.Group | null;
    local_world_frame: THREE.Group | null;

    map_center: PVF3 | null;
    map_origin: IVec;
    delta: f64;
  }): void {
    const {
      camera,
      world_frame,
      local_world_frame,
      map_center,
      map_origin,
      delta,
    } = props;

    const state = useCameraStore.getState();
    const view_state = state.view_state;
    const azimuth = view_state.azimuth;

    this._tick_camera(camera, view_state.dist, view_state.polar, delta);
    this._tick_world_rotation(world_frame, azimuth, delta);
    this._tick_local_world_translation(local_world_frame, map_center, map_origin, delta);
  }

  private _tick_camera(camera: THREE.PerspectiveCamera, next_dist: f64, next_polar: f64, delta: f64): void {
    if (!this.did_init_camera) {
      this.dist = next_dist;
      this.polar = next_polar;
      this.did_init_camera = true;
    } else {
      const alpha: f64 = 1 - Math.exp(-CAM_DAMP * delta);
      this.dist += (next_dist - this.dist) * alpha;
      this.polar += (next_polar - this.polar) * alpha;
    }

    this.dist = clamp(this.dist, MIN_DIST, MAX_DIST);
    this.polar = clamp(this.polar, MIN_POLAR, MAX_POLAR);

    const horiz_dist: f64 = this.dist * Math.sin(this.polar);
    const vert_dist: f64 = this.dist * Math.cos(this.polar);

    camera.position.set(0, -horiz_dist, vert_dist);
    camera.lookAt(0, 0, 0);
    camera.updateMatrixWorld();
  }

  private _tick_world_rotation(frame: THREE.Group | null, azimuth: f64, delta: f64): void {
    if (!frame) return;

    if (!this.did_init_rot) {
      frame.rotation.z = azimuth;
      frame.updateMatrix();
      this.did_init_rot = true;
      return;
    }

    const alpha: f64 = 1 - Math.exp(-ROT_DAMP * delta);
    const err = wrap(azimuth - frame.rotation.z, -Math.PI, Math.PI);
    frame.rotation.z += err * alpha;
    frame.updateMatrix();
  }

  private _tick_local_world_translation(
    frame: THREE.Group | null,
    map_center: PVF3 | null,
    map_origin: IVec,
    delta: f64,
  ): void {
    if (!frame || map_center === null) return;

    const did_rebase: boolean = this.prev_map_origin !== map_origin;
    this.prev_map_origin = map_origin;

    const local_i: IVec = ivec.sub(map_center.i, map_origin);

    const tx: f64 = -(ivec.to_real(ivec.X(local_i)) + fvec.to_real(fvec.X(map_center.f)));
    const ty: f64 = -(ivec.to_real(ivec.Y(local_i)) + fvec.to_real(fvec.Y(map_center.f)));
    const tz: f64 = -(ivec.to_real(ivec.Z(local_i)) + fvec.to_real(fvec.Z(map_center.f)));

    if (!this.did_init_pos || did_rebase) {
      this.did_init_pos = true;
      frame.position.set(tx, ty, tz);
      frame.updateMatrix();
      return;
    }

    const alpha: f64 = 1 - Math.exp(-POS_DAMP * delta);
    frame.position.x += (tx - frame.position.x) * alpha;
    frame.position.y += (ty - frame.position.y) * alpha;
    frame.position.z += (tz - frame.position.z) * alpha;
    frame.updateMatrix();
  }
}
