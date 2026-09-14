// @pxl/web/module/world/environment/theme.ts

import type { f64 } from "@xpute/core/kit/word.ts";

import { daylight_split, themes as THEMES } from "./theme_spec.ts";

export const enum WorldThemeKey {
  MORNING = "morning",
  DAY = "day",
  EVENING = "evening",
  NIGHT = "night",
}

export type WorldTheme = {
  fog: string;
  sky: {
    colors: {
      top: string;
      middle: string;
      horizon: string;
    };
  };
  lighting: {
    ambient: { color: string; intensity: number };
    hemisphere: { sky: string; ground: string; intensity: number };
    directional: { color: string; intensity: number };
  };
};

export const themes: Record<WorldThemeKey, WorldTheme> = THEMES;

/**
 * Theme derived from the current hour and the real sunrise/sunset for
 * wherever the map is centered (crates/pxl/core/src/world/environment.rs)
 * — replaces a fixed 05:00/21:00 split that didn't track season or
 * location. MORNING/DAY/EVENING still split the daylight span the same way
 * the old fixed version did (37.5% / 37.5% / 25%, matching its old
 * 6h/6h/4h split of a 16h fixed daylight span), just scaled to however long
 * daylight actually is today instead of always 16 hours.
 */
export function get_theme_key(hour: f64, sunrise_hour: f64, sunset_hour: f64): WorldThemeKey {
  if (hour < sunrise_hour || hour >= sunset_hour) return WorldThemeKey.NIGHT;
  const daylight_fraction = (hour - sunrise_hour) / (sunset_hour - sunrise_hour);
  if (daylight_fraction < daylight_split.MORNING_END) return WorldThemeKey.MORNING;
  if (daylight_fraction < daylight_split.DAY_END) return WorldThemeKey.DAY;
  return WorldThemeKey.EVENING;
}
