// @pxl/web/render/frame/policy.ts

import type { f64 } from "@xpute/spec/lib/kit/word.ts";
import type { F32Array } from "@xpute/spec/lib/kit/array.ts";
import { clamp, wrap } from "@xpute/spec/lib/kit/math.ts";

import { fvec, type IVec, ivec, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import { cross3, dot3, norm3, sub3, type V3 } from "@pxl/spec/space/v3.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useCameraStore } from "@/modules/camera/store.ts";
import { camera_cfg } from "@/modules/camera/config.ts";
import { render_cfg } from "@/modules/world/render/config.ts";
import { get_local_world_origin } from "@/modules/world/frame/coord.ts";

import type { TickArgs, TickOut } from "./types.ts";

const { MIN_DIST, MAX_DIST, MIN_POLAR, MAX_POLAR } = camera_cfg.view;
const { CAM_DAMP, ROT_DAMP, POS_DAMP } = camera_cfg.smooth;
const { FOV, NEAR } = camera_cfg.projection;
const { FAR_CLIP } = render_cfg;

const CAMERA_UP: V3 = [0, 0, 1];

const $x: V3 = [0, 0, 0];
const $y: V3 = [0, 0, 0];
const $z: V3 = [0, 0, 0];

function mat4_identity(): F32Array {
  const out = new Float32Array(16);
  out[0] = 1;
  out[5] = 1;
  out[10] = 1;
  out[15] = 1;
  return out;
}

function mat4_perspective(out: F32Array, fovy: f64, aspect: f64, near: f64, far: f64): F32Array {
  const f: f64 = 1.0 / Math.tan(fovy * 0.5);
  const nf: f64 = 1.0 / (near - far);

  out[0] = f / aspect;
  out[1] = 0;
  out[2] = 0;
  out[3] = 0;

  out[4] = 0;
  out[5] = f;
  out[6] = 0;
  out[7] = 0;

  out[8] = 0;
  out[9] = 0;
  out[10] = (far + near) * nf;
  out[11] = -1;

  out[12] = 0;
  out[13] = 0;
  out[14] = 2 * far * near * nf;
  out[15] = 0;

  return out;
}

function mat4_look_at(out: F32Array, eye: V3, center: V3, up: V3): F32Array {
  norm3(sub3(eye, center, $z), $z);
  norm3(cross3(up, $z, $x), $x);
  cross3($z, $x, $y);

  out[0] = $x[0];
  out[1] = $y[0];
  out[2] = $z[0];
  out[3] = 0;

  out[4] = $x[1];
  out[5] = $y[1];
  out[6] = $z[1];
  out[7] = 0;

  out[8] = $x[2];
  out[9] = $y[2];
  out[10] = $z[2];
  out[11] = 0;

  out[12] = -dot3($x, eye);
  out[13] = -dot3($y, eye);
  out[14] = -dot3($z, eye);
  out[15] = 1;

  return out;
}

function mat4_mul(out: F32Array, a: F32Array, b: F32Array): F32Array {
  const a00 = a[0], a01 = a[1], a02 = a[2], a03 = a[3];
  const a10 = a[4], a11 = a[5], a12 = a[6], a13 = a[7];
  const a20 = a[8], a21 = a[9], a22 = a[10], a23 = a[11];
  const a30 = a[12], a31 = a[13], a32 = a[14], a33 = a[15];

  const b00 = b[0], b01 = b[1], b02 = b[2], b03 = b[3];
  const b10 = b[4], b11 = b[5], b12 = b[6], b13 = b[7];
  const b20 = b[8], b21 = b[9], b22 = b[10], b23 = b[11];
  const b30 = b[12], b31 = b[13], b32 = b[14], b33 = b[15];

  out[0] = a00 * b00 + a10 * b01 + a20 * b02 + a30 * b03;
  out[1] = a01 * b00 + a11 * b01 + a21 * b02 + a31 * b03;
  out[2] = a02 * b00 + a12 * b01 + a22 * b02 + a32 * b03;
  out[3] = a03 * b00 + a13 * b01 + a23 * b02 + a33 * b03;

  out[4] = a00 * b10 + a10 * b11 + a20 * b12 + a30 * b13;
  out[5] = a01 * b10 + a11 * b11 + a21 * b12 + a31 * b13;
  out[6] = a02 * b10 + a12 * b11 + a22 * b12 + a32 * b13;
  out[7] = a03 * b10 + a13 * b11 + a23 * b12 + a33 * b13;

  out[8] = a00 * b20 + a10 * b21 + a20 * b22 + a30 * b23;
  out[9] = a01 * b20 + a11 * b21 + a21 * b22 + a31 * b23;
  out[10] = a02 * b20 + a12 * b21 + a22 * b22 + a32 * b23;
  out[11] = a03 * b20 + a13 * b21 + a23 * b22 + a33 * b23;

  out[12] = a00 * b30 + a10 * b31 + a20 * b32 + a30 * b33;
  out[13] = a01 * b30 + a11 * b31 + a21 * b32 + a31 * b33;
  out[14] = a02 * b30 + a12 * b31 + a22 * b32 + a32 * b33;
  out[15] = a03 * b30 + a13 * b31 + a23 * b32 + a33 * b33;

  return out;
}

export class FramePolicy {
  private did_init_camera = false;
  private did_init_rot = false;
  private did_init_pos = false;

  private dist: f64 = 0;
  private polar: f64 = 0;
  private world_azimuth: f64 = 0;

  private local_world_offset: V3 = [0, 0, 0];
  private prev_map_origin: IVec = 0n;

  private proj = mat4_identity();
  private view = mat4_identity();
  private view_proj = mat4_identity();
  private camera_state: TickOut["camera"] = {
    view_proj: new Float32Array(16),
    eye: [0, 0, 0],
    target: [0, 0, 0],
    up: [0, 0, 1],
    aspect: 1,
    fov_y_rad: 0,
    near: 0,
    far: 0,
  };
  private world_state: TickOut["world"] = {
    world_azimuth: 0,
    map_origin: 0n,
    local_world_offset: [0, 0, 0],
  };

  get_world_azimuth(): f64 {
    return this.world_azimuth;
  }

  tick(args: TickArgs): TickOut {
    const { width, height, dt } = args;

    const state = useCameraStore.getState();
    const view_state = state.view_state;
    const azimuth = view_state.azimuth;

    const map_center: PVF3 | null = useMapStore.getState().map_center;
    const map_origin = get_local_world_origin(map_center);

    this.tick_camera(view_state.dist, view_state.polar, dt);
    this.tick_world_rotation(azimuth, dt);
    this.tick_local_world_translation(map_center, map_origin, dt);

    const aspect: f64 = Math.max(1, width) / Math.max(1, height);

    const horiz_dist: f64 = this.dist * Math.sin(this.polar);
    const vert_dist: f64 = this.dist * Math.cos(this.polar);

    const eye = this.camera_state.eye;
    const target = this.camera_state.target;

    eye[0] = 0;
    eye[1] = -horiz_dist;
    eye[2] = vert_dist;

    target[0] = 0;
    target[1] = 0;
    target[2] = 0;

    mat4_perspective(this.proj, (FOV * Math.PI) / 180, aspect, NEAR, FAR_CLIP);
    mat4_look_at(this.view, eye, target, CAMERA_UP);
    mat4_mul(this.view_proj, this.proj, this.view);

    this.camera_state.view_proj.set(this.view_proj);
    this.camera_state.up[0] = CAMERA_UP[0];
    this.camera_state.up[1] = CAMERA_UP[1];
    this.camera_state.up[2] = CAMERA_UP[2];
    this.camera_state.aspect = aspect;
    this.camera_state.fov_y_rad = (FOV * Math.PI) / 180;
    this.camera_state.near = NEAR;
    this.camera_state.far = FAR_CLIP;

    this.world_state.world_azimuth = this.world_azimuth;
    this.world_state.map_origin = map_origin;
    this.world_state.local_world_offset[0] = this.local_world_offset[0];
    this.world_state.local_world_offset[1] = this.local_world_offset[1];
    this.world_state.local_world_offset[2] = this.local_world_offset[2];

    return {
      camera: this.camera_state,
      world: this.world_state,
    };
  }

  private tick_camera(next_dist: f64, next_polar: f64, dt: f64): void {
    if (!this.did_init_camera) {
      this.dist = next_dist;
      this.polar = next_polar;
      this.did_init_camera = true;
    } else {
      const alpha: f64 = 1 - Math.exp(-CAM_DAMP * dt);
      this.dist += (next_dist - this.dist) * alpha;
      this.polar += (next_polar - this.polar) * alpha;
    }

    this.dist = clamp(this.dist, MIN_DIST, MAX_DIST);
    this.polar = clamp(this.polar, MIN_POLAR, MAX_POLAR);
  }

  private tick_world_rotation(next_azimuth: f64, dt: f64): void {
    if (!this.did_init_rot) {
      this.world_azimuth = next_azimuth;
      this.did_init_rot = true;
      return;
    }

    const alpha: f64 = 1 - Math.exp(-ROT_DAMP * dt);
    const err = wrap(next_azimuth - this.world_azimuth, -Math.PI, Math.PI);
    this.world_azimuth += err * alpha;
  }

  private tick_local_world_translation(map_center: PVF3 | null, map_origin: IVec, dt: f64): void {
    if (map_center === null) return;

    const did_rebase: boolean = this.prev_map_origin !== map_origin;
    this.prev_map_origin = map_origin;

    const local_i: IVec = ivec.sub(map_center.i, map_origin);

    const tx: f64 = -(ivec.to_real(ivec.X(local_i)) + fvec.to_real(fvec.X(map_center.f)));
    const ty: f64 = -(ivec.to_real(ivec.Y(local_i)) + fvec.to_real(fvec.Y(map_center.f)));
    const tz: f64 = -(ivec.to_real(ivec.Z(local_i)) + fvec.to_real(fvec.Z(map_center.f)));

    if (!this.did_init_pos || did_rebase) {
      this.did_init_pos = true;
      this.local_world_offset[0] = tx;
      this.local_world_offset[1] = ty;
      this.local_world_offset[2] = tz;
      return;
    }

    const alpha: f64 = 1 - Math.exp(-POS_DAMP * dt);
    this.local_world_offset[0] += (tx - this.local_world_offset[0]) * alpha;
    this.local_world_offset[1] += (ty - this.local_world_offset[1]) * alpha;
    this.local_world_offset[2] += (tz - this.local_world_offset[2]) * alpha;
  }
}
