// @pxl/web/modules/map/ui/overlay/palette_modal.tsx

import { useTranslation } from "react-i18next";
import { X } from "lucide-react";

import { GroundChannelKey } from "@pxl/shared/bake/format.ts";
import {
  type DecorId,
  decor_ids,
  is_terrain_style_cube,
  placeable_cube_hex,
  placeable_cube_ids,
  type PlaceableCubeId,
  terrain_style_cube_source,
} from "@pxl/shared/spec/cell/mod.ts";

import type { BaseModalProps } from "@/app/types/ui.types.ts";
import { useUiStore } from "@/app/store/ui_store.ts";
import { cn } from "@/shared/lib/utils/cn.ts";
import { Sheet, SheetClose, SheetContent, SheetDescription, SheetTitle } from "@/shared/ui/shadcn/sheet.tsx";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/shared/ui/shadcn/tabs.tsx";
import { DECOR_ICONS } from "./mode_bar.tsx";

export type PaletteModalProps = {
  type: "palette";
};

/** The full catalog's own grouping — mode_bar.tsx's hotbar stays a flat
 * scrolling strip (no sections, that's the point of a hotbar), but a grid
 * with 30+ terrain-styled entries alongside the original 15 colors reads as
 * noise without headers. "colors" first (the original, most-used set), then
 * one section per cube.ts channel, in the same LAND/WATER/STRUCTURE/
 * TRANSPORT order render/layer/terrain/adapter.ts's own GROUND_CH_KEYS uses. */
const SECTION_ORDER = ["colors", GroundChannelKey.LAND, GroundChannelKey.WATER, GroundChannelKey.STRUCTURE, GroundChannelKey.TRANSPORT] as const;
type SectionKey = (typeof SECTION_ORDER)[number];

function section_of(id: PlaceableCubeId): SectionKey {
  return is_terrain_style_cube(id) ? terrain_style_cube_source(id).channel : "colors";
}

function group_by_section(ids: readonly PlaceableCubeId[]): Map<SectionKey, PlaceableCubeId[]> {
  const out = new Map<SectionKey, PlaceableCubeId[]>();
  for (const id of ids) {
    const key = section_of(id);
    const list = out.get(key);
    if (list) list.push(id);
    else out.set(key, [id]);
  }
  return out;
}

/** A terrain-styled id is `"<channel>.<LOCAL_KEY>"` (cell/mod.ts), e.g.
 * "land.GREEN_DEEP" — i18next's own default "." keySeparator resolves that
 * straight into the nested `object_colors.land.GREEN_DEEP` locale entry
 * (en.json/ko.json), no different from how a flat color id ("white")
 * resolves into `object_colors.white` sitting right beside it. */
function cube_label(t: (key: string) => string, id: PlaceableCubeId): string {
  return t(`object_colors.${id}`);
}

function CubeSwatchButton({ id, onPick }: { id: PlaceableCubeId; onPick: (id: PlaceableCubeId) => void }) {
  const { t } = useTranslation();
  const activeTool = useUiStore((state) => state.activeTool);
  const object_color_id = useUiStore((state) => state.object_color_id);
  const active = activeTool === "object" && object_color_id === id;
  const label = cube_label(t, id);

  return (
    <button
      type="button"
      onClick={() => onPick(id)}
      className="flex flex-col items-center gap-1 p-1.5 rounded-lg hover:bg-accent transition-colors cursor-pointer"
      title={label}
    >
      <span
        style={{ backgroundColor: placeable_cube_hex(id) ?? undefined }}
        className={cn(
          "w-8 h-8 rounded-md transition-transform",
          active ? "ring-2 ring-foreground ring-offset-1 ring-offset-background scale-105" : "ring-1 ring-border",
        )}
      />
      <span className="text-[10px] leading-tight text-muted-foreground text-center line-clamp-1 w-full">
        {label}
      </span>
    </button>
  );
}

function DecorGridButton({ id, onPick }: { id: DecorId; onPick: (id: DecorId) => void }) {
  const { t } = useTranslation();
  const activeTool = useUiStore((state) => state.activeTool);
  const decor_id = useUiStore((state) => state.decor_id);
  const active = activeTool === "decor" && decor_id === id;
  const Icon = DECOR_ICONS[id];

  return (
    <button
      type="button"
      onClick={() => onPick(id)}
      className="flex flex-col items-center gap-1 p-1.5 rounded-lg hover:bg-accent transition-colors cursor-pointer"
      title={t(`object_decors.${id}`)}
    >
      <span
        className={cn(
          "flex items-center justify-center w-8 h-8 rounded-md",
          active ? "bg-accent ring-2 ring-foreground" : "ring-1 ring-border",
        )}
      >
        <Icon className="w-4 h-4" strokeWidth={2} />
      </span>
      <span className="text-[10px] leading-tight text-muted-foreground text-center line-clamp-1 w-full">
        {t(`object_decors.${id}`)}
      </span>
    </button>
  );
}

/** The full cube/decor catalog — mode_bar.tsx's own hotbar (its horizontal
 * scrolling color strip + decor buttons) stays untouched and is the
 * everyday quick-pick; this is the Minecraft-inventory-style "everything"
 * view opened from its own new trigger button, never a replacement for the
 * hotbar. Picking an entry here does exactly what its hotbar counterpart
 * does (set the same ui_store fields, no separate selection state of its
 * own), then closes. */
export function PaletteModal(props: BaseModalProps & PaletteModalProps) {
  const { open, onOpenChange } = props;
  const { t } = useTranslation();
  const set_activeTool = useUiStore((state) => state.set_activeTool);
  const set_object_color_id = useUiStore((state) => state.set_object_color_id);
  const set_decor_id = useUiStore((state) => state.set_decor_id);

  const sections = group_by_section(placeable_cube_ids);

  function pick_cube(id: PlaceableCubeId): void {
    set_object_color_id(id);
    set_activeTool("object");
    onOpenChange(false);
  }

  function pick_decor(id: DecorId): void {
    set_decor_id(id);
    set_activeTool("decor");
    onOpenChange(false);
  }

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="left" hideClose className="sm:w-[360px] p-0 gap-0 flex flex-col bg-background text-foreground">
        <div className="shrink-0 border-b border-border/60" style={{ paddingTop: "env(safe-area-inset-top, 0px)" }}>
          <div className="flex h-14 items-center gap-2 px-5">
            <SheetTitle className="flex-1 text-[17px] font-semibold">{t("palette_modal.title")}</SheetTitle>
            <SheetDescription className="sr-only">{t("palette_modal.description")}</SheetDescription>
            <SheetClose
              className="-mr-2.5 flex size-[42px] items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer"
              aria-label={t("palette_modal.close")}
            >
              <X className="size-5" />
            </SheetClose>
          </div>
        </div>

        <Tabs defaultValue="cube" className="flex-1 min-h-0">
          <TabsList className="mx-5 mt-4">
            <TabsTrigger value="cube">{t("palette_modal.tabs.cube")}</TabsTrigger>
            <TabsTrigger value="decor">{t("palette_modal.tabs.decor")}</TabsTrigger>
          </TabsList>

          <div
            className="flex-1 overflow-y-auto overscroll-contain px-5 pt-4"
            style={{ paddingBottom: "calc(env(safe-area-inset-bottom, 0px) + 1rem)" }}
          >
            <TabsContent value="cube" className="flex flex-col gap-4">
              {SECTION_ORDER.map((section) => {
                const ids = sections.get(section);
                if (!ids || ids.length === 0) return null;
                return (
                  <div key={section}>
                    <h3 className="text-xs font-medium text-muted-foreground mb-2">{t(`palette_modal.sections.${section}`)}</h3>
                    <div className="grid grid-cols-5 gap-1">
                      {ids.map((id) => <CubeSwatchButton key={id} id={id} onPick={pick_cube} />)}
                    </div>
                  </div>
                );
              })}
            </TabsContent>

            <TabsContent value="decor" className="flex flex-col gap-4">
              <div className="grid grid-cols-5 gap-1">
                {decor_ids.map((id) => <DecorGridButton key={id} id={id} onPick={pick_decor} />)}
              </div>
            </TabsContent>
          </div>
        </Tabs>
      </SheetContent>
    </Sheet>
  );
}
