// @pxl/web/feature/map/ui/overlay/drawer-modal.tsx

import { useTranslation } from "react-i18next";
import { X } from "lucide-react";

import type { u8 } from "@xpute/core/abi/word.ts";
import { type ChannelName, decor_kinds, decorName, placeable_cubes, type PlaceableCube, placeableCubeChannel, placeableCubeId, sameCube } from "@/catalog/cell/mod.ts";

import type { BaseModalProps } from "@/app/type/ui.type.ts";
import { useUiStore } from "@/app/store/ui-store.ts";
import { ActiveTool } from "@/kernel/abi/cmd.spec.ts";
import { cn } from "@/shared/lib/util/cn.ts";
import { Sheet, SheetClose, SheetContent, SheetDescription, SheetTitle } from "@/shared/ui/shadcn/sheet.tsx";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/shared/ui/shadcn/tabs.tsx";
import { CharacterPicker } from "@/feature/character/picker.tsx";
import { cubeSwatchStyle } from "./cube-swatch.ts";
import { DECOR_ICONS } from "./mode-bar.tsx";

export interface DrawerModalProps {
  type: "drawer";
}

/** The full catalog's own grouping — mode-bar.tsx's hotbar stays a flat
 * scrolling strip (no sections, that's the point of a hotbar), but a grid
 * with 30+ terrain-styled entries alongside the original 15 colors reads as
 * noise without headers. A section is a cube channel and nothing else, block
 * first — the original, most-used set — then the four ground channels coarse
 * to fine, which is how a place reads: what the ground is, then what is on it.
 *
 * The order is the drawer's own. A channel's name comes from the catalog, but
 * which order a palette shows them in is a decision about this list and
 * belongs beside it. */
const SECTION_ORDER: readonly ChannelName[] = ["block", "land", "water", "structure", "transport"];

function groupBySection(cubes: readonly PlaceableCube[]): Map<ChannelName, PlaceableCube[]> {
  const out = new Map<ChannelName, PlaceableCube[]>();
  for (const cube of cubes) {
    const key = placeableCubeChannel(cube);
    const list = out.get(key);
    if (list) list.push(cube);
    else out.set(key, [cube]);
  }
  return out;
}

/** A cube's words are `"<channel>.<type>"` (cell/mod.ts), e.g.
 * "land.green_deep" or "block.white" — i18next's own default "."
 * keySeparator resolves that straight into the nested
 * `cubes.land.green_deep` locale entry (en.json/ko.json). */
function cubeLabel(t: (key: string) => string, cube: PlaceableCube): string {
  return t(`cubes.${placeableCubeId(cube)}`);
}

function CubeSwatchButton({ cube, onPick }: { cube: PlaceableCube; onPick: (cube: PlaceableCube) => void }) {
  const { t } = useTranslation();
  const activeTool = useUiStore((s) => s.activeTool);
  const held = useUiStore((s) => s.cube);
  const active = activeTool === ActiveTool.Object && sameCube(held, cube);
  const label = cubeLabel(t, cube);

  return (
    <button
      type="button"
      onClick={() => onPick(cube)}
      className="flex flex-col items-center gap-1 p-1.5 rounded-lg hover:bg-accent transition-colors cursor-pointer"
      title={label}
    >
      <span
        style={cubeSwatchStyle(cube)}
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

function DecorGridButton({ kind, onPick }: { kind: u8; onPick: (kind: u8) => void }) {
  const { t } = useTranslation();
  const activeTool = useUiStore((s) => s.activeTool);
  const held = useUiStore((s) => s.decor);
  const active = activeTool === ActiveTool.Decor && held === kind;
  const name = decorName(kind);
  const Icon = DECOR_ICONS[name];

  return (
    <button
      type="button"
      onClick={() => onPick(kind)}
      className="flex flex-col items-center gap-1 p-1.5 rounded-lg hover:bg-accent transition-colors cursor-pointer"
      title={t(`decors.${name}`)}
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
        {t(`decors.${name}`)}
      </span>
    </button>
  );
}

/** The full cube/decor/character catalog — mode-bar.tsx's own hotbar (its
 * horizontal scrolling color strip) stays untouched and is the everyday
 * quick-pick; this is the Minecraft-inventory-style "everything" view opened
 * from its own trigger button, never a replacement for the hotbar. Picking a
 * Cube/Decor entry does exactly what its (now-removed) hotbar counterpart
 * used to (set the same ui-store fields), then closes. The Character tab is
 * the odd one out: it equips the single always-on companion rather than
 * placing anything, so its picks (CharacterPicker) apply immediately and
 * deliberately leave the sheet open — see that component's own comment. */
export function DrawerModal(props: BaseModalProps & DrawerModalProps) {
  const { open, onOpenChange } = props;
  const { t } = useTranslation();
  const selectCube = useUiStore.getState().selectCube;
  const selectDecor = useUiStore.getState().selectDecor;

  const sections = groupBySection(placeable_cubes);

  function pickCube(cube: PlaceableCube): void {
    selectCube(cube);
    onOpenChange(false);
  }

  function pickDecor(kind: u8): void {
    selectDecor(kind);
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
                const cubes = sections.get(section);
                if (!cubes || cubes.length === 0) return null;
                return (
                  <div key={section}>
                    <h3 className="text-xs font-medium text-muted-foreground mb-2">{t(`drawer_modal.sections.${section}`)}</h3>
                    <div className="grid grid-cols-5 gap-1">
                      {cubes.map((cube) => <CubeSwatchButton key={placeableCubeId(cube)} cube={cube} onPick={pickCube} />)}
                    </div>
                  </div>
                );
              })}
            </TabsContent>

            <TabsContent value="decor" className="flex flex-col gap-4">
              <div className="grid grid-cols-5 gap-1">
                {decor_kinds.map((kind) => <DecorGridButton key={kind} kind={kind} onPick={pickDecor} />)}
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
