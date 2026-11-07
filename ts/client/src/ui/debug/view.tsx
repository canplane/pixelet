// @pxl/client/ui/debug/view.tsx

/**
 * The view panel: where the camera and the pointer are, read live. Shown by
 * the `view` setting, and read while shown, so its header carries where the
 * pivot is even closed.
 */

import { SETTING } from "@/ui/setting/schema.ts";
import { fixed } from "@/ui/fixed.ts";
import { Accordion } from "./accordion.tsx";
import { PerfView } from "./perf/gauge.tsx";
import { usePerfView } from "./perf/poll.ts";
import { useCore } from "@/ui/store/watch.ts";

export function ViewPanel() {
  const show = useCore((s) => s.settings[SETTING.view.key] === 1);
  const v = usePerfView(show);

  if (!show) return null;
  const note =
    v === null || Number.isNaN(v.pivot_lat)
      ? undefined
      : `${fixed(v.pivot_lat, 5)}°, ${fixed(v.pivot_lon, 5)}°`;
  return (
    <Accordion name="view" title="View" note={note}>
      {v !== null && <PerfView v={v} />}
    </Accordion>
  );
}
