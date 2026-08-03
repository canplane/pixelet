// @pxl/web/bootstrap/three.ts

import * as THREE from "three";

let booted = false;

export function bootstrap_three(): void {
  if (booted) return;
  booted = true;

  THREE.Object3D.DEFAULT_UP.set(0, 0, 1); // Z-up
}
