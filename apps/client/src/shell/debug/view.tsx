// @pxl/client/shell/debug/view.tsx

/** The view panel: where the camera and the pointer are, read live. Shown by the `view` setting. */

import { useEffect, useState } from "react";

import { useKernel } from "@/vmm/shell.ts";
import { SETTINGS } from "@/shell/setting/schema.ts";
import { Fold, useOpenFold } from "./fold.tsx";
import { ProfilerView } from "./profiler/gauge.tsx";
import { type Gauge, profilerGaugeRead } from "./profiler/view.ts";

const POLL_MS = 500;

export function ViewPanel() {
  const show = useKernel((s) => s.settings[SETTINGS.view.key] === 1);
  const open = useOpenFold((s) => s.open === "view");
  const [g, setG] = useState<Pick<Gauge, "view"> | null>(null);

  useEffect(() => {
    if (!show || !open) return;
    // No window: the view is now, not over the recording.
    const read = () => setG(profilerGaugeRead(0));
    read();
    const id = setInterval(read, POLL_MS);
    return () => clearInterval(id);
  }, [show, open]);

  if (!show) return null;
  return <Fold name="view" title="View">{g !== null && <ProfilerView v={g.view} />}</Fold>;
}
