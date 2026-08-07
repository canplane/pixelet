// @pxl/web/modules/camera/use-camera.ts

import { useEffect, useRef } from "react";
import { useFrame, useThree } from "@react-three/fiber";

import type { f64 } from "@pixelet/shared/kit/word.ts";
import { clamp } from "@pixelet/shared/kit/math.ts";

import { pvf3 } from "@pixelet/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pixelet/shared/spec/space/indexer.ts";
import { TO_XY } from "@pixelet/shared/spec/space/xy.ts";
import type { V3 } from "@pixelet/shared/spec/space/v3.ts";

import { useCameraStore } from "./store.ts";
import { camera_cfg } from "./config.ts";
import { render_cfg } from "@/modules/world/render/config.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { get_ele_at_local } from "@/modules/world/terrain/use-elevation.ts";

const { MIN_DIST, MAX_DIST, MIN_POLAR, MAX_POLAR } = camera_cfg.view;
const { CAM_DAMP } = camera_cfg.smooth;

const { FAR_CLIP } = render_cfg;

/** Minimum clearance ('u' units) kept between the camera and the ground beneath it. */
const CAMERA_GROUND_MARGIN: f64 = 2;

/** Native Z-up direction for the web world renderer. */
export const CAMERA_UP: V3 = [0, 0, 1];

export function useCamera() {
  const { camera } = useThree();

  /** Applies one-time camera runtime defaults for the Z-up scene. */
  useEffect((): void => {
    camera.up.set(...CAMERA_UP);
    camera.far = FAR_CLIP;
    camera.updateProjectionMatrix();
  }, [camera]);

  const view_state = useCameraStore((state) => state.view_state);

  const did_init = useRef(false);

  const dist = useRef<f64>(view_state.dist);
  const polar = useRef<f64>(view_state.polar);

  useFrame((_, delta) => {
    if (!did_init.current) {
      dist.current = view_state.dist;
      polar.current = view_state.polar;
      did_init.current = true;
    } else {
      const alpha: f64 = 1 - Math.exp(-CAM_DAMP * delta);
      dist.current += (view_state.dist - dist.current) * alpha;
      polar.current += (view_state.polar - polar.current) * alpha;
    }

    dist.current = clamp(dist.current, MIN_DIST, MAX_DIST);
    polar.current = clamp(polar.current, MIN_POLAR, MAX_POLAR);

    const horiz_dist: f64 = dist.current * Math.sin(polar.current);
    let vert_dist: f64 = dist.current * Math.cos(polar.current);

    // Ground-collision guard: the camera's fixed offset from map_center only accounts
    // for map_center's *own* height. Nearby terrain (a hill/cliff behind the target,
    // in whichever direction the camera currently sits at this azimuth) can rise above
    // that and clip through the camera. Only relevant when follow_terrain is grounding
    // map_center in the first place — with it off there's no elevation contract to honor.
    if (useCameraStore.getState().follow_terrain) {
      const map_center = useMapStore.getState().map_center;
      if (map_center) {
        const azimuth: f64 = view_state.azimuth;
        // Camera sits at local (0, -horiz_dist); un-rotate by azimuth to find which
        // terrain point that screen position currently coincides with (the world spins
        // under a fixed camera, so "behind the target" is azimuth-dependent).
        const local_x: f64 = -horiz_dist * Math.sin(azimuth);
        const local_y: f64 = -horiz_dist * Math.cos(azimuth);

        const origin_page_idx_xy = TO_XY(L3_IDX(map_center.i));
        const ground_h: f64 | null = get_ele_at_local(origin_page_idx_xy, local_x, local_y);
        if (ground_h !== null) {
          const center_h: f64 = pvf3.to_real(pvf3.Z(map_center));
          const min_vert: f64 = (ground_h - center_h) + CAMERA_GROUND_MARGIN;
          if (vert_dist < min_vert) vert_dist = min_vert;
        }
      }
    }

    const pos: V3 = [0, -horiz_dist, vert_dist];

    camera.position.set(pos[0], pos[1], pos[2]);
    camera.lookAt(0, 0, 0);
    camera.updateMatrixWorld();
  });
}
