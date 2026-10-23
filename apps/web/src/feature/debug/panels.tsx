// @pxl/web/feature/debug/panels.tsx

/** A panel for every debug setting, stacked in the order the settings sheet lists them: one window
 * in the corner (`feature/world/ui/corner.tsx`), which the chat's lines give way to while any of it is up. */

import type { ComponentType } from "react";

import { useKernel } from "@/kernel/shell.ts";
import { type SettingName, SETTINGS, settingsIn } from "@/feature/setting/schema.ts";
import { GizmoLegend } from "./gizmo-legend.tsx";
import { PerformanceOverlay } from "./performance.tsx";
import { ViewOverlay } from "./view.tsx";

const PANEL: Partial<Record<SettingName, ComponentType>> = {
  view_overlay: ViewOverlay,
  performance_overlay: PerformanceOverlay,
  world_gizmo: GizmoLegend,
};

/** Whether any panel is up, so the corner is the window's. */
export function useDebugWindow(): boolean {
  return useKernel((s) => settingsIn("debug").some((name) => PANEL[name] !== undefined && s.settings[SETTINGS[name].key] === 1));
}

export function DebugPanels() {
  return settingsIn("debug").map((name) => {
    const Panel = PANEL[name];
    return Panel && <Panel key={name} />;
  });
}
