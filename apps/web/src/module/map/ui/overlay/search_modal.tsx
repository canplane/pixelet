// @pxl/web/module/map/ui/overlay/search_modal.tsx

import { useEffect, useRef, useState } from "react";
import { ChevronLeft, X } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { BaseModalProps } from "@/app/type/ui.types.ts";
import { Sheet, SheetContent, SheetDescription, SheetTitle } from "@/shared/ui/shadcn/sheet.tsx";
import { TextField } from "@/shared/ui/text_field.tsx";
import { kernel_action, type SearchRow, useKernel } from "@/wasm/kernel.ts";

export type SearchModalProps = {
  type: "search";
};

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
 * (top_overlays.tsx), since "where am I" and "where do I want to go" are
 * the same mental slot for a map.
 */
export function SearchModal(props: BaseModalProps & SearchModalProps) {
  const { open, onOpenChange } = props;
  const { t } = useTranslation();
  const [query, set_query] = useState("");
  const results = useKernel((s) => s.search.results);
  const loading = useKernel((s) => s.search.loading);

  const input_ref = useRef<HTMLInputElement>(null);

  // Sheet's own open/close animation still runs on the frame `open` flips —
  // focusing synchronously in the same render would fight it.
  useEffect(() => {
    if (!open) return;
    const id = setTimeout(() => input_ref.current?.focus(), 50);
    return () => clearTimeout(id);
  }, [open]);

  // The query, its debounce and the request are the kernel's
  // (crates/pxl/core/src/module/map/ui/overlay/search_modal.rs); closing starts
  // the next search fresh.
  useEffect(() => {
    if (!open) {
      set_query("");
      kernel_action({ type: "search_close" });
    }
  }, [open]);

  function change(q: string) {
    set_query(q);
    kernel_action({ type: "search_query", query: q });
  }

  function select(result: SearchRow) {
    // The kernel grounds the jump, stops follow mode, and grounds it again
    // once the target page's terrain arrives (app/shell.rs's go_to).
    kernel_action({ type: "go_to", lon: result.lon, lat: result.lat });
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
              <TextField
                ref={input_ref}
                value={query}
                onChange={(e) => change(e.target.value)}
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
                onClick={() => change("")}
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
              <span className="text-sm truncate">{r.name}</span>
              <SubtitleRow label={r.subtitle} distance={r.distance} />
            </button>
          ))}

          {poi_results.length > 0 && <SectionHeader label={t("map.search.section_places")} />}
          {poi_results.map((r) => {
            // Same emoji/color table poi_layer.tsx's own map badge uses
            // (poi_theme.ts's own doc comment on why emoji beats a Lucide
            // glyph at this size).
            const theme = { icon: r.icon, color: r.color };
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
                  <span className="text-sm truncate">{r.name}</span>
                  <SubtitleRow label={r.subtitle} distance={r.distance} />
                </div>
              </button>
            );
          })}
        </div>
      </SheetContent>
    </Sheet>
  );
}
