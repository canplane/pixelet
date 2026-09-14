// @pxl/web/module/world/environment/theme.ts

import type { f64 } from "@xpute/core/kit/word.ts";

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

export const themes: Record<WorldThemeKey, WorldTheme> = {
  [WorldThemeKey.MORNING]: {
    fog: "#e9dccb",
    sky: {
      colors: {
        top: "#f7d9bc",
        middle: "#f9e7d2",
        horizon: "#fff6ea",
      },
    },
    lighting: {
      ambient: { color: "#fff2e3", intensity: 0.62 },
      hemisphere: { sky: "#fff3df", ground: "#d8b894", intensity: 0.78 },
      // Lowered from full strength to avoid blown-out highlights on
      // sun-facing slopes. Only the directional (sun) light is cut — ambient/
      // hemisphere are the soft fill and weren't what read as harsh. Landed
      // near 62% of the original, a ratio morning/day/evening share.
      directional: { color: "#ffd9a8", intensity: 0.45 },
    },
  },

  [WorldThemeKey.DAY]: {
    fog: "#c8def0",
    sky: {
      colors: {
        top: "#b9ddff",
        middle: "#dff1ff",
        horizon: "#f8fcff",
      },
    },
    lighting: {
      ambient: { color: "#f9fcff", intensity: 0.68 },
      hemisphere: { sky: "#edf6ff", ground: "#c2cbd3", intensity: 0.82 },
      // Directional light only, same reasoning as MORNING — ~61% of full
      // strength, in step with the other daylight themes.
      directional: { color: "#fff8ef", intensity: 0.50 },
    },
  },

  [WorldThemeKey.EVENING]: {
    fog: "#c9865f",
    sky: {
      colors: {
        top: "#6f4d4d",
        middle: "#c9865f",
        horizon: "#ffd7b0",
      },
    },
    lighting: {
      ambient: { color: "#ffe4cd", intensity: 0.58 },
      hemisphere: { sky: "#ffb874", ground: "#6a3920", intensity: 0.82 },
      // Directional light only, same pass as DAY. Landed at ~53% of full
      // strength rather than the ~61% the other two share, chosen so the
      // day(0.5)->evening->night(0.15) taper falls in roughly even steps
      // (0.15, 0.20) instead of a small step then a big one.
      directional: { color: "#ffc29d", intensity: 0.35 },
    },
  },

  [WorldThemeKey.NIGHT]: {
    fog: "#1e293b",
    sky: {
      colors: {
        top: "#0b1220",
        middle: "#172338",
        horizon: "#243248",
      },
    },
    lighting: {
      ambient: { color: "#24304a", intensity: 0.40 },
      hemisphere: { sky: "#314764", ground: "#0b1322", intensity: 0.68 },
      // The one theme where directional intensity was raised rather than
      // cut like the daytime themes above — moonlight needs to read as
      // present at all, not toned down from an already-low baseline.
      directional: { color: "#5a6d8d", intensity: 0.15 },
    },
  },
};

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
  if (daylight_fraction < 0.375) return WorldThemeKey.MORNING;
  if (daylight_fraction < 0.75) return WorldThemeKey.DAY;
  return WorldThemeKey.EVENING;
}
