// @pxl/web/modules/world/sched/gpu_timer.ts

/**
 * GPU wall time per frame, via `EXT_disjoint_timer_query_webgl2`.
 *
 * discussions/20260909-cooperative-scheduler-and-task-ledger.md reserved this
 * for "a real GPU regression that hasn't appeared", and the condition now
 * holds: frames stall for seconds with the scheduler pass at ~10ms, no mounts,
 * no re-renders and no geometry churn — every main-thread path the ledger
 * measures has been excluded, so the question is whether the time is on the
 * GPU or merely waiting.
 *
 * Chrome-desktop only by design. iOS WKWebView does not expose the extension
 * (the reason the shipped monitor must never depend on this), and Chrome
 * quantises the result, so treat it as "which order of magnitude", not a
 * precise number.
 *
 * A TIME_ELAPSED query cannot nest, so exactly one is in flight per frame and
 * results are collected a few frames later, when the GPU has caught up. A
 * `disjoint` reading means the GPU was interrupted and every outstanding
 * result is meaningless — those are dropped rather than reported as fast.
 */

type TimerExt = { TIME_ELAPSED_EXT: number; GPU_DISJOINT_EXT: number };

let gl: WebGL2RenderingContext | null = null;
let ext: TimerExt | null = null;
let active: WebGLQuery | null = null;
const pending: WebGLQuery[] = [];

/** Last completed GPU frame time in ms; 0 until one lands, or if unsupported. */
let last_gpu_ms = 0;

export function gpu_timer_supported(): boolean {
  return ext !== null;
}

export function gpu_timer_attach(context: WebGL2RenderingContext): void {
  if (gl === context) return;
  gl = context;
  ext = context.getExtension("EXT_disjoint_timer_query_webgl2") as TimerExt | null;
}

/** Opens this frame's query. No-op when unsupported or one is already open. */
export function gpu_timer_begin(): void {
  if (!gl || !ext || active) return;
  const q = gl.createQuery();
  if (!q) return;
  gl.beginQuery(ext.TIME_ELAPSED_EXT, q);
  active = q;
}

/** Closes this frame's query and harvests whatever has completed. */
export function gpu_timer_end(): void {
  if (!gl || !ext) return;
  if (active) {
    gl.endQuery(ext.TIME_ELAPSED_EXT);
    pending.push(active);
    active = null;
  }

  // A disjoint invalidates every result in flight — the GPU was preempted, so
  // the elapsed values are not comparable to anything.
  if (gl.getParameter(ext.GPU_DISJOINT_EXT)) {
    for (const q of pending) gl.deleteQuery(q);
    pending.length = 0;
    return;
  }

  while (pending.length > 0) {
    const q = pending[0];
    if (!gl.getQueryParameter(q, gl.QUERY_RESULT_AVAILABLE)) break;
    last_gpu_ms = Number(gl.getQueryParameter(q, gl.QUERY_RESULT)) / 1e6;
    gl.deleteQuery(q);
    pending.shift();
  }
}

/** Most recent completed GPU frame time, in ms. Lags a few frames. */
export function gpu_timer_last_ms(): number {
  return last_gpu_ms;
}
