// @pxl/client/ui/feature/debug/view.tsx

/** The panel is read for as long as it is shown, so its header gives the
 * pivot's position even while it is closed. */

import { SETTING } from "@/ui/shared/setting.ts";
import { fixed } from "@/ui/shared/fixed.ts";
import { Accordion } from "./accordion.tsx";
import { PerfView } from "./perf/gauge.tsx";
import { usePerfView } from "./perf/poll.ts";
import { useCore } from "@/ui/shared/watch.ts";

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
