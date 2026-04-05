// @pxl/web/bootstrap/three.ts

import * as THREE from "three/webgpu";
import { extend } from "@react-three/fiber";
import type { ThreeToJSXElements } from "@react-three/fiber";

declare module "@react-three/fiber" {
  interface ThreeElements extends ThreeToJSXElements<typeof THREE> {}
}

let booted = false;

export function bootstrap_three(): void {
  if (booted) return;
  booted = true;

  extend(THREE as any);

  THREE.Object3D.DEFAULT_UP.set(0, 0, 1); // Z-up
}
