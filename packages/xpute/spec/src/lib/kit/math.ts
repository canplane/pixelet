// @xpute/spec/lib/kit/math.ts

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

export const floor_to_step = (v: f64, step: f64): f64 => Math.floor(v / step) * step;
export const round_to_step = (v: f64, step: f64): f64 => Math.round(v / step) * step;
export const ceil_to_step = (v: f64, step: f64): f64 => Math.ceil(v / step) * step;
