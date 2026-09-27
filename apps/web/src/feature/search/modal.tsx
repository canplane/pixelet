// @pxl/web/feature/search/modal.tsx

import type { f64 } from "@xpute/core/abi/word.ts";
import { useEffect, useState } from "react";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";
import { round } from "@xpute/core/math/scalar.ts";
import { useTranslation } from "react-i18next";

import type { BaseModalProps } from "@/app/type/ui.type.ts";
import { Panel } from "@platform/panel.tsx";
import { PANEL_CONTENT_TOP, PANEL_CONTROL, PANEL_EDGE, PANEL_HEADER, PANEL_SECTION_GAP, PanelBody, PanelClose } from "@/shared/ui/panel.tsx";
import { KEEPS_KEYBOARD } from "@/shared/ui/keyboard.tsx";
import { TextField } from "@/shared/ui/text-field.tsx";
import { alpha, chrome, color, hairline, sectionTitle, textStyle, weight } from "@/shared/ui/theme.ts";
import { Command, DevicePoseStatus, encode_input_go_to } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, kernel_enable_location, kernel_snapshot } from "@/kernel/shell.ts";
import { useSensors } from "@/feature/map/ui/overlay/sensor-status.tsx";
import { LABEL, STATUS } from "@platform/palette.ts";
import { AboveKeyboard } from "@platform/above-keyboard.tsx";
import { SYSTEM_CLEAR } from "@platform/text-field.ts";
import { Backing } from "@platform/backing.tsx";
import { BackingPlace } from "@/shared/ui/backing-place.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { resolvePoiBadge } from "@/feature/search/poi-badge.ts";
import { assembleLabel } from "@/feature/map/geocode/label.ts";
import { fetchSearch, type SearchResult } from "@/feature/search/repository.ts";

// Shorter than use-reverse-geocode.ts's: that one waits for a pan to settle
// on a new page, this for typing to pause.
const DEBOUNCE_MS = 300;

interface LonLat {
  lon: f64;
  lat: f64;
}

const EARTH_RADIUS_M = 6378137;
const DEG_TO_RAD = Math.PI / 180;

/**
 * Great-circle meters between two WGS84 points.
 *
 * Not the world-unit distance scaled by epsg3857's own M_PER_U, which would
 * be cheaper and is already at hand: world units are a Mercator plane, where
 * a meter is stretched by 1/cos(latitude) — 1.26x at Seoul's — and that is
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

/** Whole meters under a kilometer, then one decimal, then whole kilometers —
 * the precision a reader can actually act on at each scale. */
function formatDistance(m: f64): string {
  if (m < 1000) return `${round(m)}m`;
  if (m < 10000) return `${(m / 1000).toFixed(1)}km`;
  return `${round(m / 1000).toLocaleString()}km`;
}

/** A result's name in the reader's language where it has one, as label.ts
 * picks a place part's. */
function displayName(result: SearchResult, lang: string): string {
  return result.common?.[lang] ?? result.name;
}

/** One result: its name, and under it where it is on the left and how far
 * away on the right. Where it is comes from its coordinates, never from a
 * source's own address text, which came in a different shape from every
 * contributor and could name another city than the point. A place has a
 * badge of its kind; an area has none, the section already says what it is. */
function ResultRow({ result, lang, distance, onPress }: { result: SearchResult; lang: string; distance: string | null; onPress: () => void }) {
  const where = result.context.length > 0 ? assembleLabel(result.context, lang) : "";
  // poi-badge.ts says why an emoji beats a glyph at this size.
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

/** The hand-drawn clear's circle, where the field has none of its own. */
const CLEAR = 16;
const LOCATE_HEIGHT = chrome.target;
/** The card's gap from the keyboard or the bottom, and from the last result. */
const LOCATE_INSET = 16;

/** Turns the location on, floating over the results while it is off: the
 * search is where "near me" is asked, and a place's distance is measured
 * from it. The compass is not asked for here — nothing reads it without the
 * location, and it is asked for where it is used, entering follow mode. */
function LocateCard() {
  const { t } = useTranslation();
  const { location } = useSensors();
  if (location === DevicePoseStatus.Ready) return null;
  return (
    <AboveKeyboard inset={LOCATE_INSET}>
      <Backing fill={color.white} place={BackingPlace.OnSheet} interactive style={styles.locateCard}>
        <Pressable onPress={kernel_enable_location} accessibilityRole="button" style={styles.locateRow}>
          <Glyph glyph="location" size={18} color={poseColor(location)} />
          <Text numberOfLines={1} style={styles.locateLabel}>{t("map.sensors.label")}</Text>
          <Text style={styles.locateAction}>{t("map.sensors.turn_on")}</Text>
        </Pressable>
      </Backing>
    </AboveKeyboard>
  );
}

/** What a query found, and where its distances are measured from: frozen at
 * the moment it ran, the point the API ranked against, so the numbers agree
 * with the order and a camera drifting behind the panel cannot renumber the
 * list being read. */
interface Found {
  query: string;
  results: SearchResult[];
  origin: LonLat | null;
}

/**
 * Forward search, which always asks the live API: nothing is baked for it
 * (repository.ts). Opened from the address title (top-overlays.tsx), since
 * where one is and where one wants to go are the same slot on a map. It
 * mounts fresh each time, so a search starts empty rather than on the last
 * place's list.
 */
export function SearchModal(props: BaseModalProps) {
  const { onClose } = props;
  const { t, i18n } = useTranslation();
  const lang = i18n.language.split("-")[0].toLowerCase();

  const [query, setQuery] = useState("");
  const [found, setFound] = useState<Found>({ query: "", results: [], origin: null });
  const q = query.trim();
  // The last list stands while the next is on its way.
  const results = q === "" ? [] : found.results;
  const loading = q !== "" && found.query !== q;

  useEffect(() => {
    if (q === "") return;
    // Latest wins: the next keystroke's query aborts this one.
    const controller = new AbortController();
    const timer = setTimeout(() => {
      // The kernel's map center, in degrees.
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
    // The kernel's map center: it grounds the jump the way a pan is, stops
    // follow mode (the user drove the camera), and grounds it again once the
    // target page's tile arrives, while the map is still on that page.
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
            {
              /* The field and its clear: a touch there leaves the keyboard up;
                one on a result, the list or anywhere else is done typing. */
            }
            <View {...KEEPS_KEYBOARD} style={styles.headerRow}>
              {
                /* The field with its magnifier and clear, then the panel's
                  close: the clear a small filled circle inside the field, the
                  close a round button of its own, so the two X's are not
                  confused. */
              }
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
                {
                  // Where the field has no clear of its own: a filled circle
                  // with an X in the paper's color, as a system search
                  // field's is.
                }
                {!SYSTEM_CLEAR && query.length > 0 && (
                  <Pressable
                    onPress={() => setQuery("")}
                    hitSlop={(chrome.target - CLEAR) / 2}
                    accessibilityRole="button"
                    accessibilityLabel={t("map.search.clear_aria_label")}
                    style={styles.clear}
                  >
                    <Glyph glyph="xmark" size={10} weight="heavy" color={color.paper} />
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

          {
            // Areas first, as the backend orders them: a name matching an
            // area is rare enough to lead whatever places contain it
            // (pxl-geo search/overture.rs). Each group has a header,
            // so the change from areas to places reads.
          }
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
  locateCard: { borderRadius: 22, pointerEvents: "auto", boxShadow: "0 6px 20px rgba(0, 0, 0, 0.12)" },
  locateRow: { height: LOCATE_HEIGHT, flexDirection: "row", alignItems: "center", gap: 10, paddingHorizontal: 16 },
  locateLabel: { flex: 1, ...textStyle.body, color: LABEL.primary },
  locateAction: { ...textStyle.body, fontWeight: weight.semibold, color: color.action },
  // Room under the last result for the card that floats over it.
  resultsRoom: { paddingTop: PANEL_CONTENT_TOP, paddingBottom: LOCATE_HEIGHT + 2 * LOCATE_INSET, gap: PANEL_SECTION_GAP },
  headerRow: { ...PANEL_HEADER, gap: chrome.controlGap },
  // The magnifier and the clear each centered on the round end it stands in,
  // concentric with the field's capsule.
  well: {
    flex: 1,
    minWidth: 0,
    height: PANEL_CONTROL,
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    paddingLeft: (PANEL_CONTROL - chrome.glyph) / 2,
    paddingRight: (PANEL_CONTROL - CLEAR) / 2,
    borderRadius: PANEL_CONTROL / 2,
  },
  field: { flex: 1, minWidth: 0, color: LABEL.primary },
  clear: { width: CLEAR, height: CLEAR, borderRadius: CLEAR / 2, alignItems: "center", justifyContent: "center", backgroundColor: LABEL.tertiary },
  results: { flex: 1 },
  empty: { paddingHorizontal: PANEL_EDGE, ...textStyle.footnote, color: LABEL.secondary },
  sectionHeader: {
    paddingHorizontal: PANEL_EDGE,
    paddingBottom: 8,
    ...sectionTitle,
    color: LABEL.secondary,
  },
  row: { flexDirection: "row", alignItems: "center", gap: 10, paddingHorizontal: PANEL_EDGE, paddingVertical: 12, borderBottomWidth: 1, borderBottomColor: hairline },
  rowText: { flex: 1, minWidth: 0, gap: 2 },
  name: { ...textStyle.body, color: LABEL.primary },
  subtitle: { flexDirection: "row", alignItems: "baseline", gap: 8 },
  small: { ...textStyle.caption, color: LABEL.secondary },
  context: { flexShrink: 1 },
  distance: { marginLeft: "auto", flexShrink: 0, fontVariant: ["tabular-nums"] },
  badge: { width: 24, height: 24, borderRadius: 12, alignItems: "center", justifyContent: "center", flexShrink: 0 },
  badgeIcon: { fontSize: textStyle.footnote.fontSize },
});
