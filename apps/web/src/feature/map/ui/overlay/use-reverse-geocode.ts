// @pxl/web/feature/map/ui/overlay/use-reverse-geocode.ts

import { useTranslation } from "react-i18next";

import { assembleLabel } from "@/feature/map/geocode/label.ts";
import { useKernel } from "@/kernel/shell.ts";

/**
 * The place the map centre is in, as one line.
 *
 * The kernel resolves it: it holds the centre, the page that centre falls
 * in, the group of pages that page belongs to and the dictionary the group's
 * numbers name places in, and it cuts the parts at the rung the current zoom
 * asks for (pxl-kernel layer/geocode/). Nothing is fetched here — the
 * debounce, the abort and the "have we asked about this page" bookkeeping
 * this hook used to carry are the kernel's I/O scheduler's.
 *
 * The parts arrive unjoined and in every language the provider had, so
 * assembleLabel() runs on every render against the current i18n.language:
 * switching UI language re-orders a label already in hand rather than
 * asking for another.
 */
export function useReverseGeocode(): string | null {
  const { i18n } = useTranslation();
  const parts = useKernel((s) => s.place);
  return parts.length > 0 ? assembleLabel(parts, i18n.language) : null;
}
