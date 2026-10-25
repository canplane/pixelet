// @pxl/client/shell/debug/view.tsx

/** The view panel: where the camera and the pointer are, read live. Shown by the `view` setting. */

import { SETTINGS } from "@/shell/setting/schema.ts";
import { Fold, useOpenFold } from "./fold.tsx";
import { ProfilerView } from "./profiler/gauge.tsx";
import { useProfilerView } from "./profiler/poll.ts";
import { useKernel } from "@/shell/store/system.ts";

export function ViewPanel() {
  const show = useKernel((s) => s.settings[SETTINGS.view.key] === 1);
  const open = useOpenFold((s) => s.open === "view");
  const v = useProfilerView(show && open);

  if (!show) return null;
  return <Fold name="view" title="View">{v !== null && <ProfilerView v={v} />}</Fold>;
}
