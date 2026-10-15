// @pxl/web/feature/debug/clock.ts

/**
 * A development page's pinned clock, so every load lights, clouds and moves
 * the world at one instant: `?at=<unix seconds>` stands the page's one clock
 * there, and with it the sky and the tick's step, which is its change. A
 * step of nothing freezes every motion the step drives — the camera's
 * approach after a drag, the wheel or a key among them — so a pinned page is
 * a fixture to measure, not one to move about in.
 */

import { set_clock } from "@/kernel/sched.ts";

export function pinClock(): void {
  const at = new URLSearchParams(location.search).get("at");
  if (at === null || !Number.isFinite(Number(at))) return;
  const ms = Number(at) * 1000;
  set_clock(() => ms);
}
