// @pxl/client/ui/feature/debug/window.tsx

import type { ComponentType } from "react";

import { SETTING, type SettingName, settingsIn } from "@/ui/shared/setting.ts";
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

const isPanel = (name: SettingName): name is PanelName =>
  Object.hasOwn(PANEL, name);

export function useDebugWindow(): boolean {
  return useCore((s) =>
    settingsIn("debug").some(
      (name) => isPanel(name) && s.settings[SETTING[name].key] === 1,
    ),
  );
}

export function DebugWindow() {
  return settingsIn("debug")
    .filter(isPanel)
    .map((name) => {
      const Panel = PANEL[name];
      return <Panel key={name} />;
    });
}
