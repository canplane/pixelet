// @pxl/client/ui/feature/debug/window.tsx

import type { ComponentType } from "react";

import { SETTING, type SettingName, settings_in } from "@/ui/shared/setting.ts";
import {
  CameraGizmoLegend,
  RayStepsLegend,
  WorldGizmoLegend,
} from "./legend.tsx";
import { PerfPanel } from "./perf.tsx";
import { ViewPanel } from "./view.tsx";
import { useCore } from "@/ui/shared/watch.ts";

const PANEL = {
  view: ViewPanel,
  perf: PerfPanel,
  world_gizmo: WorldGizmoLegend,
  camera_gizmo: CameraGizmoLegend,
  ray_step: RayStepsLegend,
} satisfies Partial<Record<SettingName, ComponentType>>;

export type PanelName = keyof typeof PANEL;

const is_panel = (name: SettingName): name is PanelName =>
  Object.hasOwn(PANEL, name);

export function useDebugWindow(): boolean {
  return useCore((s) =>
    settings_in("debug").some(
      (name) => is_panel(name) && s.settings[SETTING[name].key] === 1,
    ),
  );
}

export function DebugWindow() {
  return settings_in("debug")
    .filter(is_panel)
    .map((name) => {
      const Panel = PANEL[name];
      return <Panel key={name} />;
    });
}
