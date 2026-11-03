// @pxl/client/shell/drawer/modal.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, View } from "react-native";

import type { f64, u8 } from "@xpute/kit/abi/word.ts";
import {
  allCubes,
  type AssetId,
  type Cube,
  cubeLabel,
  cubeSection,
  decorIds,
  decorLabel,
  decorSection,
  MARKER,
} from "@/shell/catalog/asset/mod.ts";
import { PIN_LABEL } from "@/shell/catalog/asset/marker.spec.ts";

import { selectAsset, useInHand } from "@/shell/world/dock/store.ts";
import { Panel } from "@/shell/ui/panel.tsx";
import { Tabbed } from "@/shell/ui/tabbed.tsx";
import { Glyph } from "@/shell/ui/glyph.tsx";
import { chrome, color } from "@/shell/ui/theme.ts";
import { SectionTitle } from "@/shell/ui/section-title.tsx";
import { CharacterPicker } from "@/shell/drawer/character/picker.tsx";
import { CubeSwatch, decorGlyph } from "@/shell/catalog/asset/swatch.tsx";
import { LABEL } from "@/shell/ui/palette.ts";
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
  const active = useInHand(cube);

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
  const active = useInHand(kind);

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
  const active = useInHand(MARKER);

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

const CUBE_TAB = 0;
const DECOR_TAB = 1;
const MARKER_TAB = 2;
const CHARACTER_TAB = 3;

/** Cube, decor and marker picks close the sheet; character picks apply at once and leave it open. */
export function DrawerModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();

  function pick(id: AssetId): void {
    selectAsset(id);
    onClose();
  }

  return (
    <Panel label={t("drawer.title")} onClose={onClose} half>
      <NameTipLayer>
        <Tabbed
          onClose={onClose}
          tabs={[
            { value: CUBE_TAB, label: t("drawer.tabs.cube") },
            { value: DECOR_TAB, label: t("drawer.tabs.decor") },
            { value: MARKER_TAB, label: t("drawer.tabs.marker") },
            { value: CHARACTER_TAB, label: t("drawer.tabs.character") },
          ]}
        >
          {(tab) => (
            <>
              {tab === CUBE_TAB &&
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
                          onPick={pick}
                        />
                      ))
                    }
                  </Grid>
                ))}

              {tab === DECOR_TAB &&
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
                          onPick={pick}
                        />
                      ))
                    }
                  </Grid>
                ))}
              {tab === MARKER_TAB && (
                <Grid
                  head={<SectionTitle>{t("drawer.sections.pin")}</SectionTitle>}
                >
                  {(cell) => (
                    <MarkerItem cell={cell} onPick={() => pick(MARKER)} />
                  )}
                </Grid>
              )}

              {tab === CHARACTER_TAB && <CharacterPicker />}
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
