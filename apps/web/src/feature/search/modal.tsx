// @pxl/web/feature/search/modal.tsx

import type { f64 } from "@xpute/core/abi/word.ts";
import { useEffect, useRef, useState } from "react";
import { ChevronLeft, X } from "lucide-react";
import { round } from "@xpute/core/math/scalar.ts";
import { useTranslation } from "react-i18next";

import type { BaseModalProps } from "@/app/type/ui.type.ts";
import { Sheet, SheetContent, SheetDescription, SheetTitle } from "@/shared/ui/shadcn/sheet.tsx";
import { TextField } from "@/shared/ui/text-field.tsx";
import { Command, encode_input_go_to } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, kernel_snapshot } from "@/kernel/shell.ts";
import { resolvePoiTheme } from "@/feature/search/poi-theme.ts";
import { assembleLabel } from "@/feature/map/geocode/label.ts";
import { fetchSearch, type SearchResult } from "@/feature/search/repository.ts";

// Debounced like use-reverse-geocode.ts's own DEBOUNCE_MS, but shorter:
// that one waits for panning to *settle* on a new page; this waits for
// typing to pause, which is a much shorter, more familiar rhythm for a
// search box.
const DEBOUNCE_MS = 300;

export interface SearchModalProps {
  type: "search";
}

interface LonLat {
  lon: f64;
  lat: f64;
}

const EARTH_RADIUS_M = 6378137;
const DEG_TO_RAD = Math.PI / 180;

/**
 * Great-circle metres between two WGS84 points.
 *
 * Not the world-unit distance scaled by epsg3857's own M_PER_U, which would
 * be cheaper and is already at hand: world units are a Mercator plane, where
 * a metre is stretched by 1/cos(latitude) — 1.26x at Seoul's — and that is
 * not a rounding error on a label whose whole job is to say how far away
 * something is.
 */
function groundDistanceM(a: LonLat, b: LonLat): f64 {
  const lat1 = a.lat * DEG_TO_RAD, lat2 = b.lat * DEG_TO_RAD;
  const dlat = lat2 - lat1;
  const dlon = (b.lon - a.lon) * DEG_TO_RAD;
  const h = Math.sin(dlat / 2) ** 2 + Math.cos(lat1) * Math.cos(lat2) * Math.sin(dlon / 2) ** 2;
  return 2 * EARTH_RADIUS_M * Math.asin(Math.min(1, Math.sqrt(h)));
}

/** Whole metres under a kilometre, then one decimal, then whole kilometres —
 * the precision a reader can actually act on at each scale. */
function formatDistance(m: f64): string {
  if (m < 1000) return `${round(m)}m`;
  if (m < 10000) return `${(m / 1000).toFixed(1)}km`;
  return `${round(m / 1000).toLocaleString()}km`;
}

// Same "pick a language against i18n.language, fall back to the given
// name" rule label.ts's own displayPartName() applies to a PlacePart —
// SearchResult's `common` is the identical shape for exactly the same
// reason (division results share the same multilingual source).
// Never touches `name` itself for a "poi" result (always null there) or
// The subtitle is the same for both kinds: where the point actually is,
// resolved from its coordinates. It replaced Overture's own address text,
// which arrived in a different shape from every contributor and could name
// a different city than the geometry beside it.
function displayName(result: SearchResult, lang: string): string {
  return result.common?.[lang] ?? result.name;
}

/** The row under a result's name: where it is on the left, how far away on
 * the right. Rendered whenever either half has something to say, so a result
 * with no resolved context still gets its distance. */
function SubtitleRow(props: { label: string; distance: string | null }) {
  if (props.label === "" && props.distance === null) return null;
  return (
    <span className="flex items-baseline gap-2 text-xs text-muted-foreground">
      <span className="truncate">{props.label}</span>
      {props.distance !== null && <span className="ml-auto shrink-0 tabular-nums">{props.distance}</span>}
    </span>
  );
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
 * (top-overlays.tsx), since "where am I" and "where do I want to go" are
 * the same mental slot for a map.
 */
export function SearchModal(props: BaseModalProps & SearchModalProps) {
  const { open, onOpenChange } = props;
  const { t, i18n } = useTranslation();
  const lang = i18n.language.split("-")[0].toLowerCase();

  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  // Where the distances on the rows are measured from, frozen at the moment
  // the query ran rather than read live. It is the same point the API ranked
  // against, so the numbers shown always agree with the order they are shown
  // in — and a camera still drifting behind the open sheet cannot renumber a
  // list the reader is looking at.
  const [origin, setOrigin] = useState<LonLat | null>(null);
  const [loading, setLoading] = useState(false);

  const inputRef = useRef<HTMLInputElement>(null);

  // Sheet's own open/close animation still runs on the frame `open` flips —
  // focusing synchronously in the same render would fight it, so this
  // waits a tick, same as any "focus on open" pattern layered onto a
  // portal-mounted dialog.
  useEffect(() => {
    if (!open) return;
    const id = setTimeout(() => inputRef.current?.focus(), 50);
    return () => clearTimeout(id);
  }, [open]);

  // Starting fresh each time beats resuming a stale query — the last
  // thing a returning search almost never wants is last session's list
  // for a place already visited.
  useEffect(() => {
    if (!open) {
      setQuery("");
      setResults([]);
      setOrigin(null);
      setLoading(false);
    }
  }, [open]);

  useEffect(() => {
    const q = query.trim();
    if (q.length === 0) {
      setResults([]);
      setLoading(false);
      return;
    }

    setLoading(true);

    // Latest wins: the next keystroke's query aborts this one.
    const controller = new AbortController();
    const signal = controller.signal;
    const timer = setTimeout(() => {
      // The kernel's map centre, in degrees.
      const near = kernel_snapshot().map.center ?? undefined;
      fetchSearch(q, { near, signal })
        .then((found) => {
          setOrigin(near ?? null);
          setResults(found);
        })
        .catch(() => {})
        .finally(() => {
          if (!signal.aborted) setLoading(false);
        });
    }, DEBOUNCE_MS);

    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [query]);

  function select(result: SearchResult) {
    // The kernel's map centre: it grounds the jump the way a pan is, stops
    // follow mode (the user drove the camera), and grounds it again once the
    // target page's tile arrives, while the map is still on that page.
    kernel_command(Command.INPUT_GO_TO, encode_input_go_to({ lon: result.lon, lat: result.lat }));
    onOpenChange(false);
  }

  function distanceOf(result: SearchResult): string | null {
    return origin === null ? null : formatDistance(groundDistanceM(origin, { lon: result.lon, lat: result.lat }));
  }

  const divisionResults = results.filter((r) => r.kind === "division");
  const poiResults = results.filter((r) => r.kind === "poi");

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
              <TextField
                ref={inputRef}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={t("map.search.placeholder")}
                className="flex-1 min-w-0 bg-transparent outline-none"
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
                  setQuery("");
                  setResults([]);
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
            (search/provider/overture_provider.ts's own top comment: a
            name matching an admin area is rare enough to surface ahead of
            whatever POI happens to contain the same substring), and a
            section header on each group makes that ordering legible
            instead of just an unlabeled jump from areas to places. */
          }
          {divisionResults.length > 0 && <SectionHeader label={t("map.search.section_areas")} />}
          {divisionResults.map((r) => (
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
              <span className="text-sm truncate">{displayName(r, lang)}</span>
              <SubtitleRow label={r.context.length > 0 ? assembleLabel(r.context, lang) : ""} distance={distanceOf(r)} />
            </button>
          ))}

          {poiResults.length > 0 && <SectionHeader label={t("map.search.section_places")} />}
          {poiResults.map((r) => {
            // poi-theme.ts says why an emoji beats a Lucide glyph at this size.
            const theme = resolvePoiTheme(r.category);
            return (
              <button
                key={r.gers_id}
                type="button"
                onClick={() => select(r)}
                className="w-full text-left px-4 py-3 hover:bg-accent flex items-center gap-2.5 border-b border-border/40"
              >
                <span
                  className="w-6 h-6 rounded-full flex items-center justify-center text-[13px] shrink-0"
                  style={{ backgroundColor: `${theme.color}33` }}
                >
                  {theme.icon}
                </span>
                {
                  /* flex-1, or the column sizes to its own text and SubtitleRow's
                    ml-auto has nothing to push against — the distance ends up
                    beside the address instead of at the row's right edge. */
                }
                <div className="flex flex-1 flex-col gap-0.5 min-w-0">
                  <span className="text-sm truncate">{displayName(r, lang)}</span>
                  <SubtitleRow label={r.context.length > 0 ? assembleLabel(r.context, lang) : ""} distance={distanceOf(r)} />
                </div>
              </button>
            );
          })}
        </div>
      </SheetContent>
    </Sheet>
  );
}
