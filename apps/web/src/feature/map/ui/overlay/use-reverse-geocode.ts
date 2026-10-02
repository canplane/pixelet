// @pxl/web/feature/map/ui/overlay/use-reverse-geocode.ts

import { useTranslation } from "react-i18next";

import { assembleLabel } from "@/feature/map/geocode/label.ts";
import { useKernel } from "@/kernel/shell.ts";

/**
 * The place the map center is in, as one line. The kernel resolves and
 * fetches it; the label is assembled per snapshot, so a language switch
 * reorders it without asking again.
 */
export function useReverseGeocode(): string | null {
  const { i18n } = useTranslation();
  return useKernel((s) => s.place.length === 0 ? null : assembleLabel(s.place.map((p) => ({ name: p.name, names: Object.fromEntries(p.names.map((n) => [n.lang, n.name])) })), i18n.language));
}
