// @pxl/web/modules/world/render/theme_clock.ts

/**
 * Single shared source of truth for "what time-of-day theme is it right now".
 *
 * get_theme_key() in themes.ts is a pure function of the wall clock — calling
 * it directly inside a component only re-evaluates on that component's next
 * unrelated re-render, not when the clock actually crosses an hour boundary.
 * Four different components (environment.tsx, scene.tsx, user_marker.tsx,
 * html_portal.tsx) each called it independently, so the theme could go stale
 * for hours during idle viewing, and different components could show
 * different themes simultaneously if their last re-render happened to land
 * on opposite sides of an hour boundary.
 *
 * This ticks once a minute (hour-granularity theme, so that's plenty) and
 * every consumer reads the same store value, so they can't diverge.
 */

import { create } from "zustand";

import { get_theme_key, type WorldThemeKey } from "./themes.ts";

const THEME_CHECK_INTERVAL_MS = 60_000;

type ThemeClockStore = {
  theme_key: WorldThemeKey;
};

export const useThemeClockStore = create<ThemeClockStore>(() => ({
  theme_key: get_theme_key(),
}));

setInterval(() => {
  const next = get_theme_key();
  if (useThemeClockStore.getState().theme_key !== next) {
    useThemeClockStore.setState({ theme_key: next });
  }
}, THEME_CHECK_INTERVAL_MS);

/** Reactive current theme key — re-renders the caller when the hour band changes. */
export function useThemeKey(): WorldThemeKey {
  return useThemeClockStore((state) => state.theme_key);
}
