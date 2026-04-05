// // @pxl/web/modules/camera/use-camera.ts

// import { useEffect, useRef } from "react";
// import { useFrame, useThree } from "@react-three/fiber";

// import type { f64 } from "@xpute/spec/lib/kit/word.ts";
// import { clamp } from "@xpute/spec/lib/kit/math.ts";

// import type { V3 } from "@pxl/spec/space/v3.ts";

// import { useCameraStore } from "./store.ts";
// import { camera_cfg } from "./config.ts";
// import { render_cfg } from "@/modules/world/render/config.ts";

// const { MIN_DIST, MAX_DIST, MIN_POLAR, MAX_POLAR } = camera_cfg.view;
// const { CAM_DAMP } = camera_cfg.smooth;

// const { FAR_CLIP } = render_cfg;

// /** Native Z-up direction for the web world renderer. */
// export const CAMERA_UP: V3 = [0, 0, 1];

// export function useCamera() {
//   const { camera } = useThree();

//   /** Applies one-time camera runtime defaults for the Z-up scene. */
//   useEffect((): void => {
//     camera.up.set(...CAMERA_UP);
//     camera.far = FAR_CLIP;
//     camera.updateProjectionMatrix();
//   }, [camera]);

//   const view_state = useCameraStore((state) => state.view_state);

//   const did_init = useRef(false);

//   const dist = useRef<f64>(view_state.dist);
//   const polar = useRef<f64>(view_state.polar);

//   useFrame((_, delta) => {
//     if (!did_init.current) {
//       dist.current = view_state.dist;
//       polar.current = view_state.polar;
//       did_init.current = true;
//     } else {
//       const alpha: f64 = 1 - Math.exp(-CAM_DAMP * delta);
//       dist.current += (view_state.dist - dist.current) * alpha;
//       polar.current += (view_state.polar - polar.current) * alpha;
//     }

//     dist.current = clamp(dist.current, MIN_DIST, MAX_DIST);
//     polar.current = clamp(polar.current, MIN_POLAR, MAX_POLAR);

//     const horiz_dist: f64 = dist.current * Math.sin(polar.current);
//     const vert_dist: f64 = dist.current * Math.cos(polar.current);

//     const pos: V3 = [0, -horiz_dist, vert_dist];

//     camera.position.set(pos[0], pos[1], pos[2]);
//     camera.lookAt(0, 0, 0);
//     camera.updateMatrixWorld();
//   });
// }
