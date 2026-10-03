// @pxl/web/feature/search/modal.tsx

import type { f64 } from "@xpute/core/abi/word.ts";
import { useEffect, useState } from "react";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";
import { round } from "@xpute/core/math/scalar.ts";
import { useTranslation } from "react-i18next";

import { Panel } from "@platform/panel.tsx";
import { PANEL_CONTENT_TOP, PANEL_CONTROL, PANEL_EDGE, PANEL_HEADER, PANEL_SECTION_GAP, PanelBody, PanelClose } from "@/shared/ui/panel.tsx";
import { KEEPS_KEYBOARD } from "@/shared/ui/keyboard.tsx";
import { TextField } from "@/shared/ui/text-field.tsx";
import { alpha, chrome, color, hairline, sectionTitle, textStyle, weight } from "@/shared/ui/theme.ts";
import { Command, DevicePoseStatus, encode_input_go_to } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, kernel_enable_location, kernel_snapshot, useKernel } from "@/kernel/shell.ts";
import { LABEL, STATUS } from "@platform/palette.ts";
import { AboveKeyboard } from "@platform/above-keyboard.tsx";
import { SYSTEM_CLEAR } from "@platform/text-field.ts";
import { Backing } from "@platform/backing.tsx";
import { BackingPlace } from "@/shared/ui/backing-place.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { resolvePoiBadge } from "@/feature/search/poi-badge.ts";
import { assembleLabel } from "@/feature/map/geocode/label.ts";
import { fetchSearch, type SearchResult } from "@/feature/search/repository.ts";

const DEBOUNCE_MS = 300;

interface LonLat {
  lon: f64;
  lat: f64;
}

const EARTH_RADIUS_M = 6378137;
const DEG_TO_RAD = Math.PI / 180;

/** Great-circle meters, not world units scaled: Mercator stretches a meter
 * by 1/cos(latitude), 1.26x at Seoul. */
function groundDistanceM(a: LonLat, b: LonLat): f64 {
  const lat1 = a.lat * DEG_TO_RAD, lat2 = b.lat * DEG_TO_RAD;
  const dlat = lat2 - lat1;
  const dlon = (b.lon - a.lon) * DEG_TO_RAD;
  const h = Math.sin(dlat / 2) ** 2 + Math.cos(lat1) * Math.cos(lat2) * Math.sin(dlon / 2) ** 2;
  return 2 * EARTH_RADIUS_M * Math.asin(Math.min(1, Math.sqrt(h)));
}

function formatDistance(m: f64): string {
  if (m < 1000) return `${round(m)}m`;
  if (m < 10000) return `${(m / 1000).toFixed(1)}km`;
  return `${round(m / 1000).toLocaleString()}km`;
}

function displayName(result: SearchResult, lang: string): string {
  return result.common?.[lang] ?? result.name;
}

/** Where a result is comes from its coordinates, never a source's address text, which could name another city. */
function ResultRow({ result, lang, distance, onPress }: { result: SearchResult; lang: string; distance: string | null; onPress: () => void }) {
  const where = result.context.length > 0 ? assembleLabel(result.context, lang) : "";
  const badge = result.kind === "poi" ? resolvePoiBadge(result.category) : null;
  return (
    <Pressable onPress={onPress} accessibilityRole="button" style={styles.row}>
      {badge && (
        <View style={[styles.badge, { backgroundColor: alpha(badge.color, 0.2) }]}>
          <Text style={styles.badgeIcon}>{badge.icon}</Text>
        </View>
      )}
      <View style={styles.rowText}>
        <Text numberOfLines={1} style={styles.name}>{displayName(result, lang)}</Text>
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

const SECTIONS = [
  { kind: "division", label: "map.search.section_areas" },
  { kind: "poi", label: "map.search.section_places" },
] as const;

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
        <Pressable onPress={kernel_enable_location} accessibilityRole="button" style={styles.locateRow}>
          <Glyph glyph="location" size={chrome.glyph} color={poseColor(location)} />
          <Text numberOfLines={1} style={styles.locateLabel}>{t("map.sensors.label")}</Text>
          <Text style={styles.locateAction}>{t("map.sensors.turn_on")}</Text>
        </Pressable>
      </Backing>
    </AboveKeyboard>
  );
}

/** Distances are measured from the point the query ran at, so they agree
 * with the API's order while the camera drifts. */
interface Found {
  query: string;
  results: SearchResult[];
  origin: LonLat | null;
}

/** Forward search, always live: nothing is baked for it. */
export function SearchModal({ onClose }: { onClose: () => void }) {
  const { t, i18n } = useTranslation();
  const lang = i18n.language.split("-")[0].toLowerCase();

  const [query, setQuery] = useState("");
  const [found, setFound] = useState<Found>({ query: "", results: [], origin: null });
  const q = query.trim();
  const results = q === "" ? [] : found.results;
  const loading = q !== "" && found.query !== q;

  useEffect(() => {
    if (q === "") return;
    const controller = new AbortController();
    const timer = setTimeout(() => {
      const near = kernel_snapshot().map.center ?? undefined;
      fetchSearch(q, { near, signal: controller.signal })
        .then((results) => controller.signal.aborted || setFound({ query: q, results, origin: near ?? null }))
        .catch(() => controller.signal.aborted || setFound({ query: q, results: [], origin: null }));
    }, DEBOUNCE_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [q]);

  function select(result: SearchResult) {
    kernel_command(Command.INPUT_GO_TO, encode_input_go_to({ lon: result.lon, lat: result.lat }));
    onClose();
  }

  function distanceOf(result: SearchResult): string | null {
    return found.origin === null ? null : formatDistance(groundDistanceM(found.origin, { lon: result.lon, lat: result.lat }));
  }

  return (
    <Panel label={t("map.search.placeholder")} onClose={() => onClose()}>
      <PanelBody
        header={
          <>
            <View {...KEEPS_KEYBOARD} style={styles.headerRow}>
              <Backing fill={color.faint} place={BackingPlace.OnSheet} style={styles.well}>
                <Glyph glyph="magnifyingglass" size={chrome.glyph} color={LABEL.secondary} />
                <TextField
                  value={query}
                  onChangeText={setQuery}
                  placeholder={t("map.search.placeholder")}
                  placeholderTextColor={LABEL.placeholder}
                  clearButtonMode={SYSTEM_CLEAR ? "while-editing" : undefined}
                  style={styles.field}
                />
                {!SYSTEM_CLEAR && query.length > 0 && (
                  <Pressable
                    onPress={() => setQuery("")}
                    accessibilityRole="button"
                    accessibilityLabel={t("map.search.clear_aria_label")}
                    style={styles.clearReach}
                  >
                    <View style={styles.clear}>
                      <Glyph glyph="xmark" size={10} weight="heavy" color={color.ground} />
                    </View>
                  </Pressable>
                )}
              </Backing>
              <PanelClose label={t("settings.close")} onClose={() => onClose()} />
            </View>
          </>
        }
      >
        <ScrollView style={styles.results} contentContainerStyle={styles.resultsRoom} keyboardShouldPersistTaps="handled">
          {q !== "" && results.length === 0 && <Text style={styles.empty}>{loading ? t("map.search.loading") : t("map.search.no_results")}</Text>}

          {SECTIONS.map(({ kind, label }) => {
            const rows = results.filter((r) => r.kind === kind);
            if (rows.length === 0) return null;
            return (
              <View key={kind}>
                <Text style={styles.sectionHeader}>{t(label)}</Text>
                {rows.map((r) => <ResultRow key={r.gers_id} result={r} lang={lang} distance={distanceOf(r)} onPress={() => select(r)} />)}
              </View>
            );
          })}
        </ScrollView>
      </PanelBody>

      <LocateCard />
    </Panel>
  );
}

const styles = StyleSheet.create({
  locateCard: { borderRadius: chrome.target / 2, pointerEvents: "auto", boxShadow: "0 6px 20px rgba(0, 0, 0, 0.12)" },
  locateRow: { height: chrome.target, flexDirection: "row", alignItems: "center", gap: 12, paddingLeft: (chrome.target - chrome.glyph) / 2, paddingRight: PANEL_EDGE },
  locateLabel: { flex: 1, ...textStyle.body, color: LABEL.primary },
  locateAction: { ...textStyle.body, fontWeight: weight.semibold, color: color.action },
  resultsRoom: { paddingTop: PANEL_CONTENT_TOP, paddingBottom: chrome.target + 2 * PANEL_EDGE, gap: PANEL_SECTION_GAP },
  headerRow: { ...PANEL_HEADER, gap: chrome.controlGap },
  well: {
    flex: 1,
    minWidth: 0,
    height: PANEL_CONTROL,
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
    marginHorizontal: -(chrome.target - CLEAR) / 2,
    alignItems: "center",
    justifyContent: "center",
  },
  clear: { width: CLEAR, height: CLEAR, borderRadius: CLEAR / 2, alignItems: "center", justifyContent: "center", backgroundColor: LABEL.tertiary },
  results: { flex: 1 },
  empty: { paddingHorizontal: PANEL_EDGE, ...textStyle.footnote, color: LABEL.secondary },
  sectionHeader: {
    paddingHorizontal: PANEL_EDGE,
    paddingBottom: 8,
    ...sectionTitle,
    color: LABEL.secondary,
  },
  row: { flexDirection: "row", alignItems: "center", gap: 12, paddingHorizontal: PANEL_EDGE, paddingVertical: 12, borderBottomWidth: 1, borderBottomColor: hairline },
  rowText: { flex: 1, minWidth: 0, gap: 4 },
  name: { ...textStyle.body, color: LABEL.primary },
  subtitle: { flexDirection: "row", alignItems: "baseline", gap: 8 },
  small: { ...textStyle.caption, color: LABEL.secondary },
  context: { flexShrink: 1 },
  distance: { marginLeft: "auto", flexShrink: 0, fontVariant: ["tabular-nums"] },
  badge: { width: 24, height: 24, borderRadius: 12, alignItems: "center", justifyContent: "center", flexShrink: 0 },
  badgeIcon: { fontSize: textStyle.footnote.fontSize },
});
