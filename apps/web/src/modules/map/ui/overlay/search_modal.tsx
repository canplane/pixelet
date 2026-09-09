// @pxl/web/modules/map/ui/overlay/search_modal.tsx

import { useEffect, useRef, useState } from "react";
import { ChevronLeft, X } from "lucide-react";
import { useTranslation } from "react-i18next";

import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";
import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";

import type { BaseModalProps } from "@/app/types/ui.types.ts";
import { Sheet, SheetContent, SheetDescription, SheetTitle } from "@/shared/ui/shadcn/sheet.tsx";
import { useMapStore } from "@/modules/map/store.ts";
import { useCameraStore } from "@/modules/camera/store.ts";
import { ground_or_raw } from "@/modules/world/terrain/elevation.ts";
import { register_tile_interest } from "@/modules/world/terrain/store.ts";
import { resolve_poi_theme } from "@/modules/world/poi/poi_theme.ts";
import { assemble_label } from "@/modules/world/geocode/label.ts";
import { fetch_search, type SearchResult } from "@/modules/world/search/repository.ts";

// Debounced like use_reverse_geocode.ts's own DEBOUNCE_MS, but shorter:
// that one waits for panning to *settle* on a new page; this waits for
// typing to pause, which is a much shorter, more familiar rhythm for a
// search box.
const DEBOUNCE_MS = 300;

export type SearchModalProps = {
  type: "search";
};

function map_center_to_near(map_center: PVF3): { lon: number; lat: number } {
  const g = epsg4326.unproject([0, 0], map_center.i, map_center.f);
  return { lon: g[0], lat: g[1] };
}

// Same "pick a language against i18n.language, fall back to the given
// name" rule use_reverse_geocode.ts's own display_name() applies to
// GeocodePart — SearchResult's `common` is the identical shape for exactly
// the same reason (division results share the same multilingual source).
// Never touches `name` itself for a "poi" result (always null there) or
// `address` (Overture's own freeform text, shown exactly as given —
// there's no per-language variant of a single street address string).
function display_name(result: SearchResult, lang: string): string {
  return result.common?.[lang] ?? result.name;
}

function SectionHeader(props: { label: string }) {
  return (
    <div className="px-4 pt-3 pb-1.5 text-xs font-medium text-muted-foreground/70 uppercase tracking-wide">
      {props.label}
    </div>
  );
}

/**
 * Forward search — the one world/ feature with no static bake behind it
 * (repository.ts's own top comment), so this always round-trips to the
 * live API. Lives in a right-side Sheet (same ModalManager-hosted pattern
 * as SettingsModal/DrawerModal) rather than an always-visible floating bar
 * on the map itself — opened from CurrentAddress's own trigger
 * (top_overlays.tsx), since "where am I" and "where do I want to go" are
 * the same mental slot for a map.
 */
export function SearchModal(props: BaseModalProps & SearchModalProps) {
  const { open, onOpenChange } = props;
  const { t, i18n } = useTranslation();
  const lang = i18n.language.split("-")[0].toLowerCase();

  const [query, set_query] = useState("");
  const [results, set_results] = useState<SearchResult[]>([]);
  const [loading, set_loading] = useState(false);

  const input_ref = useRef<HTMLInputElement>(null);

  // Sheet's own open/close animation still runs on the frame `open` flips —
  // focusing synchronously in the same render would fight it, so this
  // waits a tick, same as any "focus on open" pattern layered onto a
  // portal-mounted dialog.
  useEffect(() => {
    if (!open) return;
    const id = setTimeout(() => input_ref.current?.focus(), 50);
    return () => clearTimeout(id);
  }, [open]);

  // Starting fresh each time beats resuming a stale query — the last
  // thing a returning search almost never wants is last session's list
  // for a place already visited.
  useEffect(() => {
    if (!open) {
      set_query("");
      set_results([]);
      set_loading(false);
    }
  }, [open]);

  useEffect(() => {
    const q = query.trim();
    if (q.length === 0) {
      set_results([]);
      set_loading(false);
      return;
    }

    const controller = new AbortController();
    set_loading(true);

    const timer = setTimeout(() => {
      (async () => {
        try {
          const map_center = useMapStore.getState().map_center;
          const near = map_center ? map_center_to_near(map_center) : undefined;
          const rows = await fetch_search(q, { near, signal: controller.signal });
          if (!controller.signal.aborted) set_results(rows);
        } catch (err) {
          if ((err as Error)?.name !== "AbortError") console.error(`[search/${q}] failed:`, err);
        } finally {
          if (!controller.signal.aborted) set_loading(false);
        }
      })();
    }, DEBOUNCE_MS);

    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [query]);

  function select(result: SearchResult) {
    // Same "user drove the camera, stop overriding it with GPS" rule
    // camera/input.tsx's own drag handlers already apply — a search jump
    // is exactly that kind of explicit user intent.
    useCameraStore.getState().set_is_follow_mode(false);

    // epsg4326.project alone leaves Z at whatever the raw projection gives
    // it (effectively 0), not real terrain height — ground_or_raw's own
    // doc comment names this exact bug for the GPS recenter button before
    // it was fixed to call this; without it the camera ends up buried
    // under the actual ground mesh at the jump target.
    //
    // Unlike pan/GPS-recenter's own ground_or_raw calls, a search jump can
    // land on a page nothing has ever loaded before, so the terrain tile
    // usually isn't there yet — ground_or_raw silently falls back to the
    // raw (unelevated) coordinate in that case (`grounded === target`
    // catches exactly that fallback, since it returns the same reference
    // rather than a newly-built grounded one). Re-ground once, the moment
    // that page's tile actually arrives, rather than leaving the camera
    // stuck underground until some unrelated interaction re-triggers a
    // ground snap.
    const target = epsg4326.project({ i: 0n, f: 0n }, [result.lon, result.lat]);
    const page_idx = L3_IDX(target.i);
    const grounded = ground_or_raw(target);
    useMapStore.getState().set_map_center(grounded);

    if (grounded === target) {
      const unregister = register_tile_interest(
        (p) => p === page_idx,
        () => {
          unregister();
          const current = useMapStore.getState().map_center;
          if (current !== null && L3_IDX(current.i) === page_idx) {
            useMapStore.getState().set_map_center(ground_or_raw(target));
          }
        },
      );
    }

    onOpenChange(false);
  }

  const division_results = results.filter((r) => r.kind === "division");
  const poi_results = results.filter((r) => r.kind === "poi");

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" hideClose className="sm:w-[380px] p-0 gap-0 flex flex-col bg-background text-foreground">
        <div className="shrink-0 border-b border-border/60" style={{ paddingTop: "env(safe-area-inset-top, 0px)" }}>
          <div className="flex h-14 items-center gap-2.5 px-4">
            {
              /* Doubles as the modal's own close trigger — a leading
              magnifying-glass icon here was purely decorative, and the
              trailing SheetClose it used to sit across from was a second
              X right next to the "clear query" X, reading as one button
              doing nothing. One `<` in the corner a search UI's own
              back-navigation slot already occupies reads unambiguously as
              "leave", with no second X anywhere in the row. */
            }
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              aria-label={t("settings.close")}
              className="-ml-2.5 flex size-[42px] items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer shrink-0"
            >
              <ChevronLeft className="size-5" />
            </button>
            <SheetTitle asChild>
              <input
                ref={input_ref}
                value={query}
                onChange={(e) => set_query(e.target.value)}
                placeholder={t("map.search.placeholder")}
                className="flex-1 min-w-0 bg-transparent outline-none text-sm"
              />
            </SheetTitle>
            <SheetDescription className="sr-only">{t("map.search.placeholder")}</SheetDescription>
            {
              /* Lucide has no filled circle-X glyph (it's a pure
              outline/stroke icon set) — composed by hand: a solid circle
              with a smaller X cut into it in the background color, the
              familiar "clear field" shape (iOS/macOS search fields) rather
              than a bare X that reads more like "close" than "clear". */
            }
            {query.length > 0 && (
              <button
                type="button"
                onClick={() => {
                  set_query("");
                  set_results([]);
                }}
                aria-label={t("map.search.clear_aria_label")}
                className="shrink-0 flex items-center justify-center size-4 rounded-full bg-muted-foreground/50 hover:bg-muted-foreground/70"
              >
                <X className="size-2.5 text-background" strokeWidth={3} />
              </button>
            )}
          </div>
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto">
          {query.trim().length > 0 && results.length === 0 && (
            <div className="px-4 py-3 text-sm text-muted-foreground">
              {loading ? t("map.search.loading") : t("map.search.no_results")}
            </div>
          )}

          {
            /* Divisions first — the backend already orders them that way
            (search/providers/overture_provider.ts's own top comment: a
            name matching an admin area is rare enough to surface ahead of
            whatever POI happens to contain the same substring), and a
            section header on each group makes that ordering legible
            instead of just an unlabeled jump from areas to places. */
          }
          {division_results.length > 0 && <SectionHeader label={t("map.search.section_areas")} />}
          {division_results.map((r) => (
            <button
              key={r.gers_id}
              type="button"
              onClick={() => select(r)}
              className="w-full text-left px-4 py-3 hover:bg-accent flex flex-col gap-0.5 border-b border-border/40"
            >
              {
                /* No icon here, unlike a POI row — a division doesn't
                have a "kind of place" to badge the way a category does,
                and the section header already says what these are. */
              }
              <span className="text-sm truncate">{display_name(r, lang)}</span>
              {r.context.length > 0 && <span className="text-xs text-muted-foreground truncate">{assemble_label(r.context, lang)}</span>}
            </button>
          ))}

          {poi_results.length > 0 && <SectionHeader label={t("map.search.section_places")} />}
          {poi_results.map((r) => {
            // Same emoji/color table poi_layer.tsx's own map badge uses
            // (poi_theme.ts's own doc comment on why emoji beats a Lucide
            // glyph at this size).
            const theme = resolve_poi_theme(r.category);
            return (
              <button
                key={r.gers_id}
                type="button"
                onClick={() => select(r)}
                className="w-full text-left px-4 py-3 hover:bg-accent flex items-start gap-2.5 border-b border-border/40"
              >
                <span
                  className="w-6 h-6 rounded-full flex items-center justify-center text-[13px] shrink-0"
                  style={{ backgroundColor: `${theme.color}33` }}
                >
                  {theme.icon}
                </span>
                <div className="flex flex-col gap-0.5 min-w-0 pt-0.5">
                  <span className="text-sm truncate">{display_name(r, lang)}</span>
                  {r.address && <span className="text-xs text-muted-foreground truncate">{r.address}</span>}
                </div>
              </button>
            );
          })}
        </div>
      </SheetContent>
    </Sheet>
  );
}
