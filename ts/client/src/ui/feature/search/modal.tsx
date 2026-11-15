// @pxl/client/ui/feature/search/modal.tsx

import type { f64 } from "@xpute/kit/abi/word.ts";
import { useEffect, useState } from "react";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";
import { Path, Svg } from "react-native-svg";
import { round } from "@xpute/kit/math/scalar.ts";
import { useTranslation } from "react-i18next";

import { Panel } from "@/ui/shared/component/panel.tsx";
import {
  PANEL_CONTENT_TOP,
  PANEL_CONTROL,
  PANEL_EDGE,
  PANEL_SECTION_GAP,
  PanelBar,
  PanelBody,
} from "@/ui/shared/component/panel.tsx";
import { KEEPS_KEYBOARD } from "@/ui/shared/component/keyboard.tsx";
import { TextField } from "@/ui/shared/component/text-field.tsx";
import {
  CHROME,
  COLOR,
  hex,
  TEXT_SCALE_CAP,
  TEXT_STYLE,
  WEIGHT,
} from "@/ui/shared/component/theme.ts";
import { SectionTitle } from "@/ui/shared/component/section-title.tsx";
import { status_color } from "@/ui/feature/world/top/sensor-icon.tsx";
import { LABEL, STATUS } from "@/ui/shared/component/palette.ts";
import { AboveKeyboard } from "@/ui/shared/component/above-keyboard.tsx";
import { SYSTEM_CLEAR } from "@/ui/shared/component/text-field.tsx";
import { Backing } from "@/ui/shared/component/backing.tsx";
import { BackingPlace } from "@/ui/shared/component/backing.tsx";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import { FILL, SYMBOL } from "./place_theme.spec.ts";
import { PLACE_SYMBOLS, type PlaceSymbol } from "./place-symbol.ts";
import { SEARCH_KIND } from "./search.spec.ts";
import { assemble_label } from "@/ui/feature/division/label.ts";
import {
  useDivisionLabel,
  usePivotCoordinate,
} from "@/ui/feature/division/pivot.ts";
import { DIVISION_SUBTYPE } from "@/ui/feature/division/division.spec.ts";
import { fetch_search, type SearchResult } from "./fetch.ts";
import { fixed } from "@/ui/shared/fixed.ts";
import { touch_outset } from "@/ui/shared/component/touch.ts";
import { useSensor } from "@/ui/shared/watch.ts";
import * as camera from "@/service/core/camera.ts";
import * as sensor from "@/service/core/sensor.ts";
import * as overlay from "@/service/core/overlay.ts";

const DEBOUNCE_MS = 300;

function format_distance(m: f64): string {
  if (m < 1000) return `${round(m)}m`;
  if (m < 10000) return `${fixed(m / 1000, 1)}km`;
  return `${round(m / 1000).toLocaleString()}km`;
}

/** A result's place is read from the divisions around its coordinates, never
 * from a source's address text, which can name another city. The row leaves
 * out the country, because the country is its section's title. */
function ResultRow({
  result,
  distance,
  onPress,
}: {
  result: SearchResult;
  distance: string | null;
  onPress: () => void;
}) {
  const within = result.context.filter(
    (p) => p.subtype !== DIVISION_SUBTYPE.COUNTRY,
  );
  const where = within.length > 0 ? assemble_label(within) : "";
  const theme =
    result.kind === SEARCH_KIND.PLACE ? place_theme(result.category) : null;
  const symbol = theme?.symbol;
  return (
    <Pressable onPress={onPress} accessibilityRole="button" style={STYLE.row}>
      {theme && (
        <View style={STYLE.badge}>
          {symbol ? (
            <Svg
              width={SYMBOL_SIZE}
              height={SYMBOL_SIZE}
              viewBox={`0 0 ${symbol.box[0]} ${symbol.box[1]}`}
            >
              {symbol.shapes.map(([d, evenOdd], i) => (
                <Path
                  key={i}
                  d={d}
                  fill={theme.fill}
                  fillRule={evenOdd ? "evenodd" : "nonzero"}
                />
              ))}
            </Svg>
          ) : (
            <View style={[STYLE.dot, { backgroundColor: theme.fill }]} />
          )}
        </View>
      )}
      <View style={STYLE.row_text}>
        <Text numberOfLines={1} style={STYLE.name}>
          {result.name}
        </Text>
        {(where !== "" || distance !== null) && (
          <View style={STYLE.subtitle}>
            <Text numberOfLines={1} style={[STYLE.small, STYLE.context]}>
              {where}
            </Text>
            {distance !== null && (
              <Text style={[STYLE.small, STYLE.distance]}>{distance}</Text>
            )}
          </View>
        )}
      </View>
    </Pressable>
  );
}

/** The country is null for a result no country holds, such as one at sea or
 * beyond the mapped divisions. */
function country_of(result: SearchResult): string | null {
  if (
    result.kind === SEARCH_KIND.DIVISION &&
    result.category === DIVISION_SUBTYPE.COUNTRY
  ) {
    return result.name;
  }
  return (
    result.context.find((p) => p.subtype === DIVISION_SUBTYPE.COUNTRY)?.name ??
    null
  );
}

interface CountrySection {
  country: string | null;
  rows: SearchResult[];
}

/** The pivot's country comes first, then each other country where its best
 * result was ranked, and the results with no country come last. Within a
 * section, results keep the API's order. */
function by_country(
  results: SearchResult[],
  home: string | null,
): CountrySection[] {
  const sections = new Map<string | null, CountrySection>();
  for (const result of results) {
    const country = country_of(result);
    const section = sections.get(country) ?? { country, rows: [] };
    section.rows.push(result);
    sections.set(country, section);
  }
  const rank = ({ country }: CountrySection) =>
    country === null ? 2 : country === home ? 0 : 1;
  return [...sections.values()].sort((a, b) => rank(a) - rank(b));
}

const CLEAR = 16;

function LocateCard() {
  const { t } = useTranslation();
  // The card offers the location until one is known, kept or fixed. A press
  // starts a watch, so while one runs, failing or not, it says it is looking
  // and takes no press.
  const location = useSensor((l) => l.state.status);
  const known = useSensor((l) => l.location !== null);
  const locating = useSensor((l) => l.is_tracking_location);
  if (known) return null;
  return (
    <AboveKeyboard inset={PANEL_EDGE}>
      <Backing
        fill={COLOR.ground}
        place={BackingPlace.ON_SHEET}
        interactive
        style={STYLE.locate_card}
      >
        <Pressable
          onPress={() => void sensor.locate()}
          disabled={locating}
          accessibilityRole="button"
          style={STYLE.locate_row}
        >
          <Glyph
            glyph="location"
            size={CHROME.glyph}
            color={status_color(location, LABEL.secondary, STATUS)}
          />
          <Text
            numberOfLines={1}
            maxFontSizeMultiplier={TEXT_SCALE_CAP.chrome}
            style={STYLE.locate_label}
          >
            {t("world.sensors.label")}
          </Text>
          <Text
            maxFontSizeMultiplier={TEXT_SCALE_CAP.chrome}
            style={locating ? STYLE.locating : STYLE.locate_action}
          >
            {locating ? t("world.search.locating") : t("world.sensors.turn_on")}
          </Text>
        </Pressable>
      </Backing>
    </AboveKeyboard>
  );
}

/** Shows where the search looks from. That is the pivot, not the device's
 * location. */
function MapCenter() {
  const { t } = useTranslation();
  const place = useDivisionLabel();
  const coordinate = usePivotCoordinate();
  if (coordinate === null) return null;
  return (
    <View>
      <SectionTitle style={STYLE.section_header}>
        {t("world.search.pivot")}
      </SectionTitle>
      <View style={STYLE.center}>
        {place !== null && (
          <Text numberOfLines={2} style={STYLE.name}>
            {place}
          </Text>
        )}
        <Text style={[STYLE.small, STYLE.numbers]}>{coordinate}</Text>
      </View>
    </View>
  );
}

/** A query's results, with `home`, the pivot's country, taken where the
 * query ran. The API measured each result's distance from that same point,
 * so the sections and the distances agree with the results' order even after
 * the camera moves on. */
interface Found {
  query: string;
  results: SearchResult[];
  home: string | null;
}

export function SearchModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();

  const [query, set_query] = useState("");
  const [found, set_found] = useState<Found>({
    query: "",
    results: [],
    home: null,
  });
  const q = query.trim();
  const results = q === "" ? [] : found.results;
  const loading = q !== "" && found.query !== q;

  useEffect(() => {
    if (q === "") return;
    const controller = new AbortController();
    const timer = setTimeout(() => {
      const shown = overlay.cur();
      const near = shown.pivot.lon_lat ?? undefined;
      const home =
        shown.division.find((p) => p.subtype === DIVISION_SUBTYPE.COUNTRY)
          ?.name ?? null;
      fetch_search(q, { near, signal: controller.signal })
        .then(
          (results) =>
            controller.signal.aborted || set_found({ query: q, results, home }),
        )
        .catch(
          () =>
            controller.signal.aborted ||
            set_found({ query: q, results: [], home: null }),
        );
    }, DEBOUNCE_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [q]);

  function select(result: SearchResult) {
    camera.goto(result.lon, result.lat);
    onClose();
  }

  function distance_of(result: SearchResult): string | null {
    return result.dist_m === null ? null : format_distance(result.dist_m);
  }

  return (
    <Panel label={t("world.search.placeholder")} onClose={onClose}>
      <PanelBody
        bar={
          <PanelBar {...KEEPS_KEYBOARD} onClose={onClose}>
            <Backing
              fill={COLOR.faint}
              place={BackingPlace.ON_SHEET}
              style={STYLE.well}
            >
              <Glyph
                glyph="magnifyingglass"
                size={CHROME.glyph}
                color={LABEL.secondary}
              />
              <TextField
                value={query}
                onChangeText={set_query}
                placeholder={t("world.search.placeholder")}
                placeholderTextColor={LABEL.placeholder}
                clearButtonMode={SYSTEM_CLEAR ? "while-editing" : undefined}
                style={STYLE.field}
              />
              {!SYSTEM_CLEAR && query.length > 0 && (
                <Pressable
                  onPress={() => set_query("")}
                  accessibilityRole="button"
                  accessibilityLabel={t("world.search.clear_aria_label")}
                  style={STYLE.clear_target}
                >
                  <View style={STYLE.clear}>
                    <Glyph
                      glyph="xmark"
                      size={10}
                      weight="heavy"
                      color={COLOR.ground}
                    />
                  </View>
                </Pressable>
              )}
            </Backing>
          </PanelBar>
        }
      >
        <ScrollView
          style={STYLE.results}
          contentContainerStyle={STYLE.results_room}
          keyboardShouldPersistTaps="handled"
        >
          {q === "" && <MapCenter />}
          {q !== "" && results.length === 0 && (
            <Text style={STYLE.empty}>
              {loading
                ? t("world.search.loading")
                : t("world.search.no_results")}
            </Text>
          )}

          {by_country(results, found.home).map(({ country, rows }) => (
            <View key={country ?? ""}>
              <SectionTitle style={STYLE.section_header}>
                {country === null
                  ? t("world.search.section_elsewhere")
                  : country}
              </SectionTitle>
              {rows.map((r) => (
                <ResultRow
                  key={r.gers_id}
                  result={r}
                  distance={distance_of(r)}
                  onPress={() => select(r)}
                />
              ))}
            </View>
          ))}
        </ScrollView>
      </PanelBody>

      <LocateCard />
    </Panel>
  );
}

const STYLE = StyleSheet.create({
  locate_card: {
    borderRadius: CHROME.target / 2,
    pointerEvents: "auto",
    boxShadow: "0 6px 20px rgba(0, 0, 0, 0.12)",
  },
  locate_row: {
    minHeight: CHROME.target,
    flexDirection: "row",
    alignItems: "center",
    gap: 12,
    paddingLeft: (CHROME.target - CHROME.glyph) / 2,
    paddingRight: PANEL_EDGE,
  },
  locate_label: { flex: 1, ...TEXT_STYLE.body, color: LABEL.primary },
  locate_action: {
    ...TEXT_STYLE.body,
    fontWeight: WEIGHT.semibold,
    color: COLOR.action,
  },
  locating: { ...TEXT_STYLE.body, color: LABEL.secondary },
  results_room: {
    paddingTop: PANEL_CONTENT_TOP,
    paddingBottom: CHROME.target + 2 * PANEL_EDGE,
    gap: PANEL_SECTION_GAP,
  },
  well: {
    flex: 1,
    minWidth: 0,
    minHeight: PANEL_CONTROL,
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    paddingLeft: (PANEL_CONTROL - CHROME.glyph) / 2,
    paddingRight: (PANEL_CONTROL - CLEAR) / 2,
    borderRadius: PANEL_CONTROL / 2,
  },
  field: { flex: 1, minWidth: 0, color: LABEL.primary },
  clear_target: {
    width: CHROME.target,
    height: CHROME.target,
    marginHorizontal: -touch_outset(CLEAR),
    alignItems: "center",
    justifyContent: "center",
  },
  clear: {
    width: CLEAR,
    height: CLEAR,
    borderRadius: CLEAR / 2,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: LABEL.tertiary,
  },
  results: { flex: 1 },
  empty: {
    paddingHorizontal: PANEL_EDGE,
    ...TEXT_STYLE.footnote,
    color: LABEL.secondary,
  },
  section_header: { paddingHorizontal: PANEL_EDGE, paddingBottom: 8 },
  row: {
    flexDirection: "row",
    alignItems: "center",
    gap: 12,
    paddingHorizontal: PANEL_EDGE,
    paddingVertical: 12,
  },
  row_text: { flex: 1, minWidth: 0, gap: 4 },
  name: { ...TEXT_STYLE.body, color: LABEL.primary },
  subtitle: { flexDirection: "row", alignItems: "baseline", gap: 8 },
  small: { ...TEXT_STYLE.caption, color: LABEL.secondary },
  context: { flexShrink: 1 },
  center: { gap: 4, paddingHorizontal: PANEL_EDGE },
  numbers: { fontVariant: ["tabular-nums"] },
  distance: {
    marginLeft: "auto",
    flexShrink: 0,
    fontVariant: ["tabular-nums"],
  },
  badge: {
    width: 24,
    height: 24,
    borderRadius: 12,
    alignItems: "center",
    justifyContent: "center",
    flexShrink: 0,
  },
  dot: { width: 10, height: 10, borderRadius: 5 },
});

/** How Carto draws a place of `category`, the same way the world draws it
 * (place_theme.spec.ts): its symbol in its fill color, or a dot of that fill
 * for a category with no symbol. It is null for a category Carto draws no
 * point for. */
function place_theme(
  category: number,
): { fill: string; symbol: PlaceSymbol | undefined } | null {
  const fill = FILL[category - 1];
  if (fill === null || fill === undefined) return null;
  return { fill: hex(fill), symbol: PLACE_SYMBOLS[SYMBOL[category - 1] - 1] };
}

const SYMBOL_SIZE = 18;
