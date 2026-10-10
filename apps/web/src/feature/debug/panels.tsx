// @pxl/web/feature/debug/panels.tsx

/** A panel for every debug setting, stacked in the order the settings sheet lists them. */

import type { ComponentType } from "react";

import { useKernel } from "@/kernel/shell.ts";
import { type SettingName, SETTINGS, settingsIn } from "@/feature/setting/schema.ts";
import { Fold } from "./fold.tsx";
import { GizmoLegend } from "./gizmo-legend.tsx";
import { PerformanceOverlay } from "./performance.tsx";
import { ViewOverlay } from "./view.tsx";

const PANEL: Partial<Record<SettingName, ComponentType>> = {
  view_overlay: ViewOverlay,
  performance_overlay: PerformanceOverlay,
  world_gizmo: GizmoLegend,
  camera_gizmo: CameraGizmo,
};

/** Nothing to say yet: the grid is on screen. */
function CameraGizmo() {
  const show = useKernel((s) => s.settings[SETTINGS.camera_gizmo.key] === 1);
  return show && <Fold name="camera_gizmo" title="Camera Gizmo" />;
}

export function DebugPanels() {
  return settingsIn("debug").map((name) => {
    const Panel = PANEL[name];
    return Panel && <Panel key={name} />;
  });
}
