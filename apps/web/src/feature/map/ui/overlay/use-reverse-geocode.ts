// @pxl/web/feature/map/ui/overlay/use-reverse-geocode.ts

import { useTranslation } from "react-i18next";

import { assembleLabel } from "@/feature/map/geocode/label.ts";
import { useKernel } from "@/kernel/shell.ts";

/**
 * The place the map center is in, as one line.
 *
 * The kernel resolves it: it holds the center, the page that center falls
 * in, the group of pages that page belongs to and the dictionary the group's
 * numbers name places in, and it cuts the parts at the rung the current zoom
 * asks for (pxl-kernel layer/geocode/). Nothing is fetched here — the
 * debounce, the abort and the "have we asked about this page" bookkeeping
 * are the kernel's I/O scheduler's.
 *
 * The parts arrive unjoined and in every language the provider had, so
 * assembleLabel() runs on every snapshot against the current i18n.language:
 * switching UI language re-orders a label already in hand rather than
 * asking for another. A packet carries a part's languages as a list; the
 * label reads them by tag, as the search API gives them.
 */
export function useReverseGeocode(): string | null {
  const { i18n } = useTranslation();
  return useKernel((s) => s.place.length === 0 ? null : assembleLabel(s.place.map((p) => ({ name: p.name, names: Object.fromEntries(p.names.map((n) => [n.lang, n.name])) })), i18n.language));
}
