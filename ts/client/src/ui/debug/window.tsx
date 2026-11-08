// @pxl/client/ui/debug/window.tsx

/**
 * The debug window: a panel for each debug setting that has one, stacked in
 * the order the settings sheet lists the settings. The window takes the
 * corner (ui/world/corner.tsx), and the chat's lines give way to it while
 * any panel is shown.
 */

import type { ComponentType } from "react";

import { SETTING, type SettingName, settingsIn } from "@/ui/setting/schema.ts";
import { WorldGizmoLegend } from "./legend.tsx";
import { PerfPanel } from "./perf.tsx";
import { ViewPanel } from "./view.tsx";
import { useCore } from "@/ui/watch.ts";

const PANEL = {
  view: ViewPanel,
  perf: PerfPanel,
  world_gizmo: WorldGizmoLegend,
} satisfies Partial<Record<SettingName, ComponentType>>;

/**
 * A debug setting that has a panel. The accordion (accordion.tsx) tells its
 * panels apart by this name.
 */
export type PanelName = keyof typeof PANEL;

const isPanel = (name: SettingName): name is PanelName =>
  Object.hasOwn(PANEL, name);

/** Whether any panel's setting is on. While one is, the corner shows the
 * debug window. */
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
