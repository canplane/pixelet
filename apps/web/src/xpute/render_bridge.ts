// @pxl/web/xpute/render_bridge.ts
//
// Singleton bridge: sends an OffscreenCanvas from the main thread
// to the CORE Worker for engine.init_render().

export type RenderBridge = {
  send_canvas(canvas: OffscreenCanvas): void;
};

let _bridge: RenderBridge | null = null;

export function set_render_bridge(bridge: RenderBridge): void {
  _bridge = bridge;
}

/** Transfer the OffscreenCanvas to the CORE Worker. Call once after canvas is mounted. */
export function send_render_canvas(canvas: OffscreenCanvas): void {
  if (!_bridge) {
    console.warn("render_bridge: not ready yet");
    return;
  }
  _bridge.send_canvas(canvas);
}
