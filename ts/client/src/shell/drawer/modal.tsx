// @pxl/client/shell/drawer/modal.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, View } from "react-native";

import type { f64, u8 } from "@xpute/kit/abi/word.ts";
import {
  allCubes,
  type Cube,
  cubeLabel,
  cubeSection,
  decorIds,
  decorLabel,
  decorSection,
} from "@/shell/catalog/asset/mod.ts";
import { PIN_LABEL } from "@/shell/catalog/asset/marker.spec.ts";

import {
  selectCube,
  selectDecor,
  selectMarker,
  useInHand,
} from "@/shell/world/dock/store.ts";
import { ActiveTool } from "@/abi/cmd.spec.ts";
import { Panel } from "@/shell/ui/panel.tsx";
import { Tabbed } from "@/shell/ui/tabbed.tsx";
import { Glyph } from "@/shell/ui/glyph.tsx";
import { chrome, color } from "@/shell/ui/theme.ts";
import { SectionTitle } from "@/shell/ui/section-title.tsx";
import { CharacterPicker } from "@/shell/drawer/character/picker.tsx";
import { CubeSwatch, decorGlyph } from "@/shell/catalog/asset/swatch.tsx";
import { LABEL } from "@/shell/ui/palette.ts";
import { useKernel } from "@/shell/store/watch.ts";
import { Grid, Item, itemOf, mark } from "./grid.tsx";
import { NameTipLayer } from "./name-tip.tsx";

/** Each section's items, in the catalog's order. */
function sectioned<S, T>(
  items: readonly T[],
  sectionOf: (item: T) => S,
): Map<S, T[]> {
  const sections = new Map<S, T[]>();
  for (const item of items) {
    sections.set(sectionOf(item), [
      ...(sections.get(sectionOf(item)) ?? []),
      item,
    ]);
  }
  return sections;
}
const SECTIONS = sectioned(allCubes, cubeSection);
const DECOR_SECTIONS = sectioned(decorIds, decorSection);

// A circle cut a block's texture away, so a rounded square.
const ITEM_RADIUS = 12;

function CubeItem({
  cube,
  cell,
  onPick,
}: {
  cube: Cube;
  cell: f64;
  onPick: (cube: Cube) => void;
}) {
  const active = useInHand({ tool: ActiveTool.Cube, cube });

  return (
    <Item
      label={cubeLabel(cube)}
      active={active}
      cell={cell}
      onPress={() => onPick(cube)}
    >
      <CubeSwatch
        cube={cube}
        size={itemOf(cell)}
        radius={ITEM_RADIUS}
        style={active ? mark.on : mark.off}
      />
    </Item>
  );
}

function DecorItem({
  kind,
  cell,
  onPick,
}: {
  kind: u8;
  cell: f64;
  onPick: (kind: u8) => void;
}) {
  const active = useInHand({ tool: ActiveTool.Decor, kind });

  return (
    <Item
      label={decorLabel(kind)}
      active={active}
      cell={cell}
      onPress={() => onPick(kind)}
    >
      <View
        style={[
          styles.decor,
          { width: itemOf(cell), height: itemOf(cell) },
          active ? [mark.on, styles.decorOn] : mark.off,
        ]}
      >
        <Glyph
          glyph={decorGlyph(kind)}
          size={chrome.glyph}
          color={LABEL.primary}
        />
      </View>
    </Item>
  );
}

/** A temporary way in: one marker, its words not kept yet. */
function MarkerItem({ cell, onPick }: { cell: f64; onPick: () => void }) {
  const active = useKernel(({ tool: h }) => h.active === ActiveTool.Marker);

  return (
    <Item label={PIN_LABEL} active={active} cell={cell} onPress={onPick}>
      <View
        style={[
          styles.decor,
          { width: itemOf(cell), height: itemOf(cell) },
          active ? [mark.on, styles.decorOn] : mark.off,
        ]}
      >
        <Glyph glyph="bubble.left" size={chrome.glyph} color={LABEL.primary} />
      </View>
    </Item>
  );
}

const CUBE = 0;
const DECOR = 1;
const MARKER = 2;
const CHARACTER = 3;

/** Cube, decor and marker picks close the sheet; character picks apply at once and leave it open. */
export function DrawerModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();

  function pickCube(cube: Cube): void {
    selectCube(cube);
    onClose();
  }

  function pickDecor(kind: u8): void {
    selectDecor(kind);
    onClose();
  }

  function pickMarker(): void {
    selectMarker();
    onClose();
  }

  return (
    <Panel label={t("drawer.title")} onClose={onClose} half>
      <NameTipLayer>
        <Tabbed
          onClose={onClose}
          tabs={[
            { value: CUBE, label: t("drawer.tabs.cube") },
            { value: DECOR, label: t("drawer.tabs.decor") },
            { value: MARKER, label: t("drawer.tabs.marker") },
            { value: CHARACTER, label: t("drawer.tabs.character") },
          ]}
        >
          {(tab) => (
            <>
              {tab === CUBE &&
                [...SECTIONS].map(([section, cubes]) => (
                  <Grid
                    key={section}
                    head={
                      <SectionTitle>
                        {t(`drawer.sections.cube.${section}`)}
                      </SectionTitle>
                    }
                  >
                    {(cell) =>
                      cubes.map((cube) => (
                        <CubeItem
                          key={cube}
                          cube={cube}
                          cell={cell}
                          onPick={pickCube}
                        />
                      ))
                    }
                  </Grid>
                ))}

              {tab === DECOR &&
                [...DECOR_SECTIONS].map(([section, kinds]) => (
                  <Grid
                    key={section}
                    head={
                      <SectionTitle>
                        {t(`drawer.sections.decor.${section}`)}
                      </SectionTitle>
                    }
                  >
                    {(cell) =>
                      kinds.map((kind) => (
                        <DecorItem
                          key={kind}
                          kind={kind}
                          cell={cell}
                          onPick={pickDecor}
                        />
                      ))
                    }
                  </Grid>
                ))}
              {tab === MARKER && (
                <Grid
                  head={<SectionTitle>{t("drawer.sections.pin")}</SectionTitle>}
                >
                  {(cell) => <MarkerItem cell={cell} onPick={pickMarker} />}
                </Grid>
              )}

              {tab === CHARACTER && <CharacterPicker />}
            </>
          )}
        </Tabbed>
      </NameTipLayer>
    </Panel>
  );
}

const styles = StyleSheet.create({
  decor: {
    borderRadius: ITEM_RADIUS,
    alignItems: "center",
    justifyContent: "center",
  },
  decorOn: { backgroundColor: color.faint },
});
