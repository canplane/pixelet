// @pxl/web/modules/world/render/themes.ts

import type { u32 } from "@xpute/core/kit/word.ts";

import type { V3 } from "@pxl/shared/spec/space/v3.ts";

export const enum WorldThemeKey {
  MORNING = "morning",
  DAY = "day",
  EVENING = "evening",
  NIGHT = "night",
}

export type WorldTheme = {
  background: string;
  fog: string;
  sky: {
    sun_position: V3; // ENU [X, Y, Z]
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
  grid: {
    cell: string;
    major: string;
  };
};

export const themes: Record<WorldThemeKey, WorldTheme> = {
  [WorldThemeKey.MORNING]: {
    background: "#f6efe6",
    fog: "#e9dccb",
    sky: {
      sun_position: [100, 20, 16],
      colors: {
        top: "#f7d9bc",
        middle: "#f9e7d2",
        horizon: "#fff6ea",
      },
    },
    lighting: {
      ambient: { color: "#fff2e3", intensity: 0.62 },
      hemisphere: { sky: "#fff3df", ground: "#d8b894", intensity: 0.78 },
      directional: { color: "#ffd9a8", intensity: 0.72 },
    },
    grid: { cell: "#ddd2c4", major: "#c5b6a4" },
  },

  [WorldThemeKey.DAY]: {
    background: "#dff1ff",
    fog: "#c8def0",
    sky: {
      sun_position: [0, -50, 82],
      colors: {
        top: "#b9ddff",
        middle: "#dff1ff",
        horizon: "#f8fcff",
      },
    },
    lighting: {
      ambient: { color: "#f9fcff", intensity: 0.68 },
      hemisphere: { sky: "#edf6ff", ground: "#c2cbd3", intensity: 0.82 },
      directional: { color: "#fff8ef", intensity: 0.82 },
    },
    grid: { cell: "#c7d4df", major: "#9fb0bf" },
  },

  [WorldThemeKey.EVENING]: {
    background: "#3c2318",
    fog: "#c9865f",
    sky: {
      sun_position: [-100, -20, 12],
      colors: {
        top: "#6f4d4d",
        middle: "#c9865f",
        horizon: "#ffd7b0",
      },
    },
    lighting: {
      ambient: { color: "#ffe4cd", intensity: 0.58 },
      hemisphere: { sky: "#ffb874", ground: "#6a3920", intensity: 0.82 },
      directional: { color: "#ffc29d", intensity: 0.66 },
    },
    grid: { cell: "#8c5a3d", major: "#5e3927" },
  },

  [WorldThemeKey.NIGHT]: {
    background: "#0f172a",
    fog: "#1e293b",
    sky: {
      sun_position: [0, 0, -100],
      colors: {
        top: "#0b1220",
        middle: "#172338",
        horizon: "#243248",
      },
    },
    lighting: {
      ambient: { color: "#24304a", intensity: 0.40 },
      hemisphere: { sky: "#314764", ground: "#0b1322", intensity: 0.68 },
      directional: { color: "#5a6d8d", intensity: 0.12 },
    },
    grid: { cell: "#314258", major: "#223247" },
  },
};

/** Theme derived from current virtual hour. */
export function get_theme_key(hour: u32 = new Date().getHours()): WorldThemeKey {
  if (hour >= 5 && hour < 11) return WorldThemeKey.MORNING;
  if (hour >= 11 && hour < 17) return WorldThemeKey.DAY;
  if (hour >= 17 && hour < 21) return WorldThemeKey.EVENING;
  return WorldThemeKey.NIGHT;
}
