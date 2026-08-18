// @pxl/web/modules/map/ui/overlay/use_reverse_geocode.ts

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";
import type { G4326 } from "@pxl/shared/spec/space/adapter/4326.ts";
import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { GEOCODE_BASE_URL } from "@/lib/env/config.ts";

// Nominatim's public-instance usage policy caps this at 1 req/sec — keep this
// well above that floor if changed, or requests risk getting throttled/banned.
const DEBOUNCE_MS = 1500;

const G4326_SCRATCH: G4326 = [0, 0, 0];

const DISPLAY_NAME_SEGMENTS = 3;

export type ReverseGeocodeResult = {
  label: string | null;
  loading: boolean;
};

// zoom=14 is deliberately coarse (neighborhood-level, not a full street address) —
// raising it would resolve to exact building/lot numbers.
export function useReverseGeocode(map_center: PVF3 | null): ReverseGeocodeResult {
  const { i18n } = useTranslation();
  const [label, set_label] = useState<string | null>(null);
  const [loading, set_loading] = useState(false);

  const resolved_key_ref = useRef<string | null>(null);

  useEffect(() => {
    if (map_center === null) return;

    // Scoped to the whole effect, not just the setTimeout callback, so cleanup can
    // abort an in-flight fetch too — otherwise a newer map_center couldn't cancel an
    // older request still on the wire, and responses could resolve out of order.
    const controller = new AbortController();

    const timer = setTimeout(() => {
      const [lng, lat] = epsg4326.unproject(G4326_SCRATCH, map_center.i, map_center.f);
      const key = `${lng.toFixed(4)},${lat.toFixed(4)},${i18n.language}`;
      if (key === resolved_key_ref.current) return;
      resolved_key_ref.current = key;

      set_loading(true);

      (async () => {
        try {
          const url = `${GEOCODE_BASE_URL}/reverse?format=jsonv2&lat=${lat}&lon=${lng}&zoom=14&accept-language=${i18n.language}`;
          const res = await fetch(url, { signal: controller.signal });
          if (!res.ok) throw new Error(`reverse geocode http ${res.status}`);

          const data = await res.json();
          const display_name: string | undefined = data?.display_name;
          const segments = display_name?.split(",").map((s) => s.trim()).filter(Boolean) ?? [];
          set_label(segments.length > 0 ? segments.slice(0, DISPLAY_NAME_SEGMENTS).join(", ") : null);
        } catch (err) {
          if ((err as Error)?.name !== "AbortError") {
            console.error("[reverse_geocode] failed:", err);
          }
        } finally {
          if (!controller.signal.aborted) set_loading(false);
        }
      })();
    }, DEBOUNCE_MS);

    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [map_center, i18n.language]);

  return { label, loading };
}
