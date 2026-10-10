// @pxl/web/feature/debug/panels.tsx

/** The debug panels, stacked in the order the settings sheet lists the settings that show them. */

import type { ComponentType } from "react";

import { type SettingName, settingsIn } from "@/feature/setting/schema.ts";
import { GizmoLegend } from "./gizmo-legend.tsx";
import { PerformanceOverlay } from "./performance.tsx";
import { ViewOverlay } from "./view.tsx";

const PANEL: Partial<Record<SettingName, ComponentType>> = {
  view_overlay: ViewOverlay,
  performance_overlay: PerformanceOverlay,
  world_gizmo: GizmoLegend,
};

export function DebugPanels() {
  return settingsIn("debug").map((name) => {
    const Panel = PANEL[name];
    return Panel && <Panel key={name} />;
  });
}
