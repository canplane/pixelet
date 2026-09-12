// @pxl/web/module/map/ui/overlay/drawer_modal.tsx

import { useTranslation } from "react-i18next";
import { X } from "lucide-react";

import { GroundChannelKey } from "@pxl/shared/bake/format.ts";
import { decor_ids, type DecorId, is_terrain_style_cube, placeable_cube_ids, type PlaceableCubeId, terrain_style_cube_source } from "@pxl/shared/spec/cell/mod.ts";

import type { BaseModalProps } from "@/app/type/ui.types.ts";
import { useUiStore } from "@/app/store/ui_store.ts";
import { cn } from "@/shared/lib/util/cn.ts";
import { Sheet, SheetClose, SheetContent, SheetDescription, SheetTitle } from "@/shared/ui/shadcn/sheet.tsx";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/shared/ui/shadcn/tabs.tsx";
import { CharacterPicker } from "@/module/world/character/character_picker.tsx";
import { cube_swatch_style } from "./cube_swatch.ts";
import { DECOR_ICONS } from "./mode_bar.tsx";

export type DrawerModalProps = {
  type: "drawer";
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
        style={cube_swatch_style(id)}
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

/** The full cube/decor/character catalog — mode_bar.tsx's own hotbar (its
 * horizontal scrolling color strip) stays untouched and is the everyday
 * quick-pick; this is the Minecraft-inventory-style "everything" view opened
 * from its own trigger button, never a replacement for the hotbar. Picking a
 * Cube/Decor entry does exactly what its (now-removed) hotbar counterpart
 * used to (set the same ui_store fields), then closes. The Character tab is
 * the odd one out: it equips the single always-on companion rather than
 * placing anything, so its picks (CharacterPicker) apply immediately and
 * deliberately leave the sheet open — see that component's own comment. */
export function DrawerModal(props: BaseModalProps & DrawerModalProps) {
  const { open, onOpenChange } = props;
  const { t } = useTranslation();
  const select_cube = useUiStore((state) => state.select_cube);
  const select_decor = useUiStore((state) => state.select_decor);

  const sections = group_by_section(placeable_cube_ids);

  function pick_cube(id: PlaceableCubeId): void {
    select_cube(id);
    onOpenChange(false);
  }

  function pick_decor(id: DecorId): void {
    select_decor(id);
    onOpenChange(false);
  }

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" hideClose className="sm:w-[360px] p-0 gap-0 flex flex-col bg-background text-foreground">
        <div className="shrink-0 border-b border-border/60" style={{ paddingTop: "env(safe-area-inset-top, 0px)" }}>
          <div className="flex h-14 items-center gap-2 px-5">
            <SheetTitle className="flex-1 text-[17px] font-semibold">{t("drawer_modal.title")}</SheetTitle>
            <SheetDescription className="sr-only">{t("drawer_modal.description")}</SheetDescription>
            <SheetClose
              className="-mr-2.5 flex size-[42px] items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer"
              aria-label={t("drawer_modal.close")}
            >
              <X className="size-5" />
            </SheetClose>
          </div>
        </div>

        <Tabs defaultValue="cube" className="flex-1 min-h-0">
          <TabsList className="mx-5 mt-4">
            <TabsTrigger value="cube">{t("drawer_modal.tabs.cube")}</TabsTrigger>
            <TabsTrigger value="decor">{t("drawer_modal.tabs.decor")}</TabsTrigger>
            <TabsTrigger value="character">{t("drawer_modal.tabs.character")}</TabsTrigger>
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
                    <h3 className="text-xs font-medium text-muted-foreground mb-2">{t(`drawer_modal.sections.${section}`)}</h3>
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

            <TabsContent value="character">
              <CharacterPicker />
            </TabsContent>
          </div>
        </Tabs>
      </SheetContent>
    </Sheet>
  );
}
