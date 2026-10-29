// @pxl/client/shell/debug/window.tsx

/** A panel for every debug setting, stacked in the order the settings sheet lists them: one window
 * in the corner (`shell/world/corner.tsx`), which the chat's lines give way to while any of it is up. */

import type { ComponentType } from "react";

import {
  SETTING,
  type SettingName,
  settingsIn,
} from "@/shell/setting/schema.ts";
import { Fold } from "./fold.tsx";
import { GizmoLegend } from "./gizmo-legend.tsx";
import { ProfilerPanel } from "./profiler.tsx";
import { ViewPanel } from "./view.tsx";
import { useKernel } from "@/shell/store/watch.ts";

const PANEL = {
  view: ViewPanel,
  profiler: ProfilerPanel,
  world_gizmo: GizmoLegend,
  camera_gizmo: CameraGizmo,
} satisfies Partial<Record<SettingName, ComponentType>>;

/**
 * A debug setting that has a panel, which its accordion panel is named by
 * (accordion.tsx).
 */
export type PanelName = keyof typeof PANEL;

const isPanel = (name: SettingName): name is PanelName =>
  Object.hasOwn(PANEL, name);

/** Nothing to say yet: the grid is on screen. */
function CameraGizmo() {
  const show = useKernel((s) => s.settings[SETTINGS.camera_gizmo.key] === 1);
  return show && <Fold name="camera_gizmo" title="Camera Gizmo" />;
}

/** Whether any panel is up, so the corner is the window's. */
export function useDebugWindow(): boolean {
  return useKernel((s) =>
    settingsIn("debug").some((name) =>
      isPanel(name) && s.settings[SETTING[name].key] === 1
    )
  );
}

export function DebugWindow() {
  return settingsIn("debug").filter(isPanel).map((name) => {
    const Panel = PANEL[name];
    return <Panel key={name} />;
  });
}
