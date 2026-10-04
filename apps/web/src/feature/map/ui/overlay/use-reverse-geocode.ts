// @pxl/web/feature/map/ui/overlay/use-reverse-geocode.ts

import { useTranslation } from "react-i18next";

import { assembleLabel } from "@/feature/map/geocode/label.ts";
import { useKernel } from "@/kernel/shell.ts";

/** Assembled per snapshot, so a language switch needs no refetch. */
export function useReverseGeocode(): string | null {
  const { i18n } = useTranslation();
  const parts = useKernel((s) => s.place);
  if (parts.length === 0) return null;
  return assembleLabel(parts.map((p) => ({ name: p.name, names: Object.fromEntries(p.names.map((n) => [n.lang, n.name])) })), i18n.language);
}
