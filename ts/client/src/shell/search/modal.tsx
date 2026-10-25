// @pxl/client/shell/search/modal.tsx

import type { f64 } from "@xpute/kit/abi/word.ts";
import { useEffect, useState } from "react";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";
import { Path, Svg } from "react-native-svg";
import { round } from "@xpute/kit/math/scalar.ts";
import { useTranslation } from "react-i18next";

import { Panel } from "@/shell/ui/panel.tsx";
import { PANEL_CONTENT_TOP, PANEL_CONTROL, PANEL_EDGE, PANEL_SECTION_GAP, PanelBar, PanelBody } from "@/shell/ui/panel.tsx";
import { KEEPS_KEYBOARD } from "@/shell/ui/keyboard.tsx";
import { TextField } from "@/shell/ui/text-field.tsx";
import { chrome, color, textScale, textStyle, weight } from "@/shell/ui/theme.ts";
import { SectionTitle } from "@/shell/ui/section-title.tsx";
import { Command, DevicePoseStatus, encode_input_go_to } from "@/abi/cmd.spec.ts";
import { kernel_command, kernel_room, kernel_snapshot } from "@/abi/shell.ts";
import { LABEL, STATUS } from "@/shell/ui/palette.ts";
import { AboveKeyboard } from "@/shell/ui/above-keyboard.tsx";
import { SYSTEM_CLEAR } from "@/shell/ui/text-field.tsx";
import { Backing } from "@/shell/ui/backing.tsx";
import { BackingPlace } from "@/shell/ui/backing.tsx";
import { Glyph } from "@/shell/ui/glyph.tsx";
import { THEME } from "./place_theme.spec.ts";
import { PLACE_SYMBOLS } from "./place-symbol.ts";
import { assembleLabel, localName, type Name, primaryLanguage } from "@/shell/division/label.ts";
import { useCenterCoordinates, useDivisionLabel } from "@/shell/division/focus.ts";
import { DIVISION_SUBTYPE } from "@/shell/division/division.spec.ts";
import { fetchSearch, type SearchResult } from "./fetch.ts";
import { fixed } from "@/shell/fixed.ts";
import { touchReach } from "@/shell/ui/touch.ts";
import { useKernel } from "@/shell/store/watch.ts";

const DEBOUNCE_MS = 300;

function formatDistance(m: f64): string {
  if (m < 1000) return `${round(m)}m`;
  if (m < 10000) return `${fixed(m / 1000, 1)}km`;
  return `${round(m / 1000).toLocaleString()}km`;
}

/** Where a result is comes from its coordinates, never a source's address text, which could name another city.
 * The country is its section's title, so the row leaves it out. */
function ResultRow({ result, lang, distance, onPress }: { result: SearchResult; lang: string; distance: string | null; onPress: () => void }) {
  const within = result.context.filter((p) => p.subtype !== DIVISION_SUBTYPE.COUNTRY);
  const where = within.length > 0 ? assembleLabel(within, lang) : "";
  const theme = result.kind === "place" ? placeTheme(result.category) : null;
  const symbol = theme?.symbol ? PLACE_SYMBOLS[theme.symbol] : undefined;
  return (
    <Pressable onPress={onPress} accessibilityRole="button" style={styles.row}>
      {theme && (
        <View style={styles.badge}>
          {symbol
            ? (
              <Svg width={SYMBOL_SIZE} height={SYMBOL_SIZE} viewBox={`0 0 ${symbol.box[0]} ${symbol.box[1]}`}>
                {symbol.shapes.map(([d, evenOdd], i) => <Path key={i} d={d} fill={theme.fill} fillRule={evenOdd ? "evenodd" : "nonzero"} />)}
              </Svg>
            )
            : <View style={[styles.dot, { backgroundColor: theme.fill }]} />}
        </View>
      )}
      <View style={styles.rowText}>
        <Text numberOfLines={1} style={styles.name}>{localName({ name: result.name, names: result.common }, lang)}</Text>
        {(where !== "" || distance !== null) && (
          <View style={styles.subtitle}>
            <Text numberOfLines={1} style={[styles.small, styles.context]}>{where}</Text>
            {distance !== null && <Text style={[styles.small, styles.distance]}>{distance}</Text>}
          </View>
        )}
      </View>
    </Pressable>
  );
}

/** The country a result is in, or is; null for one no country holds, at sea or past the divisions. */
function countryOf(result: SearchResult): Name | null {
  if (result.kind === "division" && result.category === "country") return { name: result.name, names: result.common };
  return result.context.find((p) => p.subtype === DIVISION_SUBTYPE.COUNTRY) ?? null;
}

interface CountrySection {
  country: Name | null;
  rows: SearchResult[];
}

/** By country, a country by the provider's own name: the focus's first, as
 * the API ranks it, then each where its best result ranked, none last. */
function byCountry(results: SearchResult[], home: string | null): CountrySection[] {
  const sections = new Map<string | null, CountrySection>();
  for (const result of results) {
    const country = countryOf(result);
    const key = country?.name ?? null;
    const section = sections.get(key) ?? { country, rows: [] };
    section.rows.push(result);
    sections.set(key, section);
  }
  const rank = ({ country }: CountrySection) => country === null ? 2 : country.name === home ? 0 : 1;
  return [...sections.values()].sort((a, b) => rank(a) - rank(b));
}

function poseColor(status: DevicePoseStatus): string {
  switch (status) {
    case DevicePoseStatus.Loading:
      return STATUS.pending;
    case DevicePoseStatus.Error:
      return STATUS.failed;
    default:
      return LABEL.secondary;
  }
}

const CLEAR = 16;

function LocateCard() {
  const { t } = useTranslation();
  const location = useKernel((s) => s.location.status);
  if (location === DevicePoseStatus.Ready) return null;
  return (
    <AboveKeyboard inset={PANEL_EDGE}>
      <Backing fill={color.ground} place={BackingPlace.OnSheet} interactive style={styles.locateCard}>
        <Pressable onPress={() => kernel_command(Command.INPUT_LOCATE)} accessibilityRole="button" style={styles.locateRow}>
          <Glyph glyph="location" size={chrome.glyph} color={poseColor(location)} />
          <Text numberOfLines={1} maxFontSizeMultiplier={textScale.chrome} style={styles.locateLabel}>{t("world.sensors.label")}</Text>
          <Text maxFontSizeMultiplier={textScale.chrome} style={styles.locateAction}>{t("world.sensors.turn_on")}</Text>
        </Pressable>
      </Backing>
    </AboveKeyboard>
  );
}

/** Where the search looks from: the focus, which is not the device's location. */
function MapCenter() {
  const { t } = useTranslation();
  const place = useDivisionLabel();
  const coordinates = useCenterCoordinates();
  if (coordinates === null) return null;
  return (
    <View>
      <SectionTitle style={styles.sectionHeader}>{t("world.search.focus")}</SectionTitle>
      <View style={styles.center}>
        {place !== null && <Text numberOfLines={2} style={styles.name}>{place}</Text>}
        <Text style={[styles.small, styles.numbers]}>{coordinates}</Text>
      </View>
    </View>
  );
}

/** The focus's country is taken where the query ran, as the API measured each
 * result's distance from, so both agree with its order while the camera drifts. */
interface Found {
  query: string;
  results: SearchResult[];
  home: string | null;
}

/** Forward search, always live: nothing is baked for it. */
export function SearchModal({ onClose }: { onClose: () => void }) {
  const { t, i18n } = useTranslation();
  const lang = primaryLanguage(i18n.language);

  const [query, setQuery] = useState("");
  const [found, setFound] = useState<Found>({ query: "", results: [], home: null });
  const q = query.trim();
  const results = q === "" ? [] : found.results;
  const loading = q !== "" && found.query !== q;

  useEffect(() => {
    if (q === "") return;
    const controller = new AbortController();
    const timer = setTimeout(() => {
      const snapshot = kernel_snapshot();
      const near = snapshot.focus.center ?? undefined;
      const home = snapshot.division.find((p) => p.subtype === DIVISION_SUBTYPE.COUNTRY)?.name ?? null;
      fetchSearch(q, { near, signal: controller.signal })
        .then((results) => controller.signal.aborted || setFound({ query: q, results, home }))
        .catch(() => controller.signal.aborted || setFound({ query: q, results: [], home: null }));
    }, DEBOUNCE_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [q]);

  function select(result: SearchResult) {
    kernel_command(Command.INPUT_GO_TO, encode_input_go_to(kernel_room, result.lon, result.lat));
    onClose();
  }

  function distanceOf(result: SearchResult): string | null {
    return result.distance_m === null ? null : formatDistance(result.distance_m);
  }

  return (
    <Panel label={t("world.search.placeholder")} onClose={onClose}>
      <PanelBody
        bar={
          <PanelBar {...KEEPS_KEYBOARD} onClose={onClose}>
            <Backing fill={color.faint} place={BackingPlace.OnSheet} style={styles.well}>
              <Glyph glyph="magnifyingglass" size={chrome.glyph} color={LABEL.secondary} />
              <TextField
                value={query}
                onChangeText={setQuery}
                placeholder={t("world.search.placeholder")}
                placeholderTextColor={LABEL.placeholder}
                clearButtonMode={SYSTEM_CLEAR ? "while-editing" : undefined}
                style={styles.field}
              />
              {!SYSTEM_CLEAR && query.length > 0 && (
                <Pressable
                  onPress={() => setQuery("")}
                  accessibilityRole="button"
                  accessibilityLabel={t("world.search.clear_aria_label")}
                  style={styles.clearReach}
                >
                  <View style={styles.clear}>
                    <Glyph glyph="xmark" size={10} weight="heavy" color={color.ground} />
                  </View>
                </Pressable>
              )}
            </Backing>
          </PanelBar>
        }
      >
        <ScrollView style={styles.results} contentContainerStyle={styles.resultsRoom} keyboardShouldPersistTaps="handled">
          {q === "" && <MapCenter />}
          {q !== "" && results.length === 0 && <Text style={styles.empty}>{loading ? t("world.search.loading") : t("world.search.no_results")}</Text>}

          {byCountry(results, found.home).map(({ country, rows }) => (
            <View key={country?.name ?? ""}>
              <SectionTitle style={styles.sectionHeader}>{country === null ? t("world.search.section_elsewhere") : localName(country, lang)}</SectionTitle>
              {rows.map((r) => <ResultRow key={r.gers_id} result={r} lang={lang} distance={distanceOf(r)} onPress={() => select(r)} />)}
            </View>
          ))}
        </ScrollView>
      </PanelBody>

      <LocateCard />
    </Panel>
  );
}

const styles = StyleSheet.create({
  locateCard: { borderRadius: chrome.target / 2, pointerEvents: "auto", boxShadow: "0 6px 20px rgba(0, 0, 0, 0.12)" },
  locateRow: { minHeight: chrome.target, flexDirection: "row", alignItems: "center", gap: 12, paddingLeft: (chrome.target - chrome.glyph) / 2, paddingRight: PANEL_EDGE },
  locateLabel: { flex: 1, ...textStyle.body, color: LABEL.primary },
  locateAction: { ...textStyle.body, fontWeight: weight.semibold, color: color.action },
  resultsRoom: { paddingTop: PANEL_CONTENT_TOP, paddingBottom: chrome.target + 2 * PANEL_EDGE, gap: PANEL_SECTION_GAP },
  well: {
    flex: 1,
    minWidth: 0,
    minHeight: PANEL_CONTROL,
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    paddingLeft: (PANEL_CONTROL - chrome.glyph) / 2,
    paddingRight: (PANEL_CONTROL - CLEAR) / 2,
    borderRadius: PANEL_CONTROL / 2,
  },
  field: { flex: 1, minWidth: 0, color: LABEL.primary },
  clearReach: {
    width: chrome.target,
    height: chrome.target,
    marginHorizontal: -touchReach(CLEAR),
    alignItems: "center",
    justifyContent: "center",
  },
  clear: { width: CLEAR, height: CLEAR, borderRadius: CLEAR / 2, alignItems: "center", justifyContent: "center", backgroundColor: LABEL.tertiary },
  results: { flex: 1 },
  empty: { paddingHorizontal: PANEL_EDGE, ...textStyle.footnote, color: LABEL.secondary },
  sectionHeader: { paddingHorizontal: PANEL_EDGE, paddingBottom: 8 },
  row: { flexDirection: "row", alignItems: "center", gap: 12, paddingHorizontal: PANEL_EDGE, paddingVertical: 12 },
  rowText: { flex: 1, minWidth: 0, gap: 4 },
  name: { ...textStyle.body, color: LABEL.primary },
  subtitle: { flexDirection: "row", alignItems: "baseline", gap: 8 },
  small: { ...textStyle.caption, color: LABEL.secondary },
  context: { flexShrink: 1 },
  center: { gap: 4, paddingHorizontal: PANEL_EDGE },
  numbers: { fontVariant: ["tabular-nums"] },
  distance: { marginLeft: "auto", flexShrink: 0, fontVariant: ["tabular-nums"] },
  badge: { width: 24, height: 24, borderRadius: 12, alignItems: "center", justifyContent: "center", flexShrink: 0 },
  dot: { width: 10, height: 10, borderRadius: 5 },
});

/** What Carto draws a place's kind as, as the map does
 * (`place_theme.spec.ts`): its symbol in its fill, or a dot of the fill for
 * a kind with none; nothing for a kind Carto draws no point for. */
function placeTheme(category: string): { fill: string; symbol?: string | null } | null {
  return (THEME as Record<string, { fill: string; symbol?: string | null }>)[category] ?? null;
}

const SYMBOL_SIZE = 18;
