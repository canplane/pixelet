// @pxl/client/shell/debug/view.tsx

/**
 * The view panel: where the camera and the pointer are, read live. Shown by
 * the `view` setting, and read while shown, so its header carries where the
 * pivot is even closed.
 */

import { SETTING } from "@/shell/setting/schema.ts";
import { fixed } from "@/shell/fixed.ts";
import { Accordion } from "./accordion.tsx";
import { ProfilerView } from "./profiler/gauge.tsx";
import { useProfilerView } from "./profiler/poll.ts";
import { useKernel } from "@/shell/store/watch.ts";

export function ViewPanel() {
  const show = useKernel((s) => s.settings[SETTING.view.key] === 1);
  const v = useProfilerView(show);

  if (!show) return null;
  const note = v === null || Number.isNaN(v.pivot_lat)
    ? undefined
    : `${fixed(v.pivot_lat, 5)}°, ${fixed(v.pivot_lon, 5)}°`;
  return (
    <Accordion name="view" title="View" note={note}>
      {v !== null && <ProfilerView v={v} />}
    </Accordion>
  );
}
