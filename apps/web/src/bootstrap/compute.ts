// // @pxl/web/bootstrap/compute.ts

// import * as tf from "@tensorflow/tfjs-core";
// import "@tensorflow/tfjs-backend-webgpu";

// let booted = false;

// export async function bootstrap_compute(): Promise<void> {
//   if (booted) return;
//   booted = true;

//   await tf.setBackend("webgpu");
//   await tf.ready();
// }
