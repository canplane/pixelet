// @pxl/web/feature/search/modal.tsx

import type { f64 } from "@xpute/core/abi/word.ts";
import { useEffect, useState } from "react";
import { ChevronLeft, Locate, X } from "lucide-react-native";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";
import { round } from "@xpute/core/math/scalar.ts";
import { useTranslation } from "react-i18next";

import type { BaseModalProps } from "@/app/type/ui.type.ts";
import { useSafeArea } from "@platform/safe-area.ts";
import { Panel, PANEL_HEADER_PX, Side } from "@/shared/ui/panel.tsx";
import { KEEPS_KEYBOARD } from "@/shared/ui/keyboard.tsx";
import { TextField } from "@/shared/ui/text-field.tsx";
import { alpha, color, hairline, sectionTitle, textStyle, weight } from "@/shared/ui/theme.ts";
import { Command, DevicePoseStatus, encode_input_go_to } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, kernel_enable_location, kernel_snapshot } from "@/kernel/shell.ts";
import { useSensors } from "@/feature/map/ui/overlay/sensor-status.tsx";
import { LABEL, STATUS } from "@platform/palette.ts";
import { AboveKeyboard } from "@platform/above-keyboard.tsx";
import { Glass } from "@platform/glass.tsx";
import { resolvePoiBadge } from "@/feature/search/poi-badge.ts";
import { assembleLabel } from "@/feature/map/geocode/label.ts";
import { fetchSearch, type SearchResult } from "@/feature/search/repository.ts";

// Debounced like use-reverse-geocode.ts's own DEBOUNCE_MS, but shorter:
// that one waits for panning to *settle* on a new page; this waits for
// typing to pause, which is a much shorter, more familiar rhythm for a
// search box.
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

// Same "pick a language against i18n.language, fall back to the given
// name" rule label.ts's own displayPartName() applies to a PlacePart —
// SearchResult's `common` is the identical shape for exactly the same
// reason (division results share the same multilingual source).
// Never touches `name` itself for a "poi" result (always null there) or
// The subtitle is the same for both kinds: where the point actually is,
// resolved from its coordinates. It replaced Overture's own address text,
// which arrived in a different shape from every contributor and could name
// a different city than the geometry beside it.
function displayName(result: SearchResult, lang: string): string {
  return result.common?.[lang] ?? result.name;
}

/** The row under a result's name: where it is on the left, how far away on
 * the right. Rendered whenever either half has something to say, so a result
 * with no resolved context still gets its distance. */
function SubtitleRow(props: { label: string; distance: string | null }) {
  if (props.label === "" && props.distance === null) return null;
  return (
    <View style={styles.subtitle}>
      <Text numberOfLines={1} style={[styles.small, styles.context]}>{props.label}</Text>
      {props.distance !== null && <Text style={[styles.small, styles.distance]}>{props.distance}</Text>}
    </View>
  );
}

function SectionHeader(props: { label: string }) {
  return <Text style={styles.sectionHeader}>{props.label}</Text>;
}

/**
 * Forward search — the one world/ feature with no static bake behind it
 * (repository.ts's own top comment), so this always round-trips to the
 * live API. Lives in a right-side panel (same ModalManager-hosted pattern
 * as SettingsModal/DrawerModal) rather than an always-visible floating bar
 * on the map itself — opened from CurrentAddress's own trigger
 * (top-overlays.tsx), since "where am I" and "where do I want to go" are
 * the same mental slot for a map. It is mounted fresh each time it opens,
 * so a returning search starts empty: the last thing it almost never wants
 * is last session's list for a place already visited.
 */

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

/** Turns the location on, floating over the results while it is off: the
 * search is where "near me" is asked, and a place's distance is measured
 * from it. The compass is not asked for here — nothing reads it without the
 * location, and it is asked for where it is used, entering follow mode. */
function LocateCard() {
  const { t } = useTranslation();
  const { location } = useSensors();
  if (location === DevicePoseStatus.Ready) return null;
  return (
    <AboveKeyboard inset={16}>
      <Glass fill={color.white} interactive style={styles.locateCard}>
        <Pressable onPress={kernel_enable_location} accessibilityRole="button" style={styles.locateRow}>
          <Locate size={18} color={poseColor(location)} />
          <Text numberOfLines={1} style={styles.locateLabel}>{t("map.sensors.label")}</Text>
          <Text style={styles.locateAction}>{t("map.sensors.turn_on")}</Text>
        </Pressable>
      </Glass>
    </AboveKeyboard>
  );
}

export function SearchModal(props: BaseModalProps) {
  const { onClose } = props;
  const { t, i18n } = useTranslation();
  const { top } = useSafeArea();
  const lang = i18n.language.split("-")[0].toLowerCase();

  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  // Where the distances on the rows are measured from, frozen at the moment
  // the query ran rather than read live. It is the same point the API ranked
  // against, so the numbers shown always agree with the order they are shown
  // in — and a camera still drifting behind the open panel cannot renumber a
  // list the reader is looking at.
  const [origin, setOrigin] = useState<LonLat | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const q = query.trim();
    if (q.length === 0) {
      setResults([]);
      setLoading(false);
      return;
    }

    setLoading(true);

    // Latest wins: the next keystroke's query aborts this one.
    const controller = new AbortController();
    const signal = controller.signal;
    const timer = setTimeout(() => {
      // The kernel's map center, in degrees.
      const near = kernel_snapshot().map.center ?? undefined;
      fetchSearch(q, { near, signal })
        .then((found) => {
          setOrigin(near ?? null);
          setResults(found);
        })
        .catch(() => {})
        .finally(() => {
          if (!signal.aborted) setLoading(false);
        });
    }, DEBOUNCE_MS);

    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [query]);

  function select(result: SearchResult) {
    // The kernel's map center: it grounds the jump the way a pan is, stops
    // follow mode (the user drove the camera), and grounds it again once the
    // target page's tile arrives, while the map is still on that page.
    kernel_command(Command.INPUT_GO_TO, encode_input_go_to({ lon: result.lon, lat: result.lat }));
    onClose();
  }

  function distanceOf(result: SearchResult): string | null {
    return origin === null ? null : formatDistance(groundDistanceM(origin, { lon: result.lon, lat: result.lat }));
  }

  const divisionResults = results.filter((r) => r.kind === "division");
  const poiResults = results.filter((r) => r.kind === "poi");

  return (
    <Panel side={Side.Right} width={380} label={t("map.search.placeholder")} onClose={() => onClose()} dismissOnOutside>
      <View style={[styles.header, { paddingTop: top }]}>
        {
          /* The field and its clear: a touch there leaves the keyboard up;
            one on a result, the list or anywhere else is done typing. */
        }
        <View {...KEEPS_KEYBOARD} style={styles.headerRow}>
          {
            /* Doubles as the panel's own close trigger — a leading
            magnifying-glass icon here was purely decorative, and a trailing
            close beside it was a second X right next to the "clear query"
            X, reading as one button doing nothing. One `<` in the corner a
            search UI's own back-navigation slot already occupies reads
            unambiguously as "leave", with no second X anywhere in the row. */
          }
          <Pressable onPress={() => onClose()} accessibilityRole="button" accessibilityLabel={t("settings.close")} style={styles.back}>
            <ChevronLeft size={20} color={color.textMuted} />
          </Pressable>
          <TextField
            value={query}
            onChangeText={setQuery}
            placeholder={t("map.search.placeholder")}
            placeholderTextColor={color.textMuted}
            style={styles.field}
          />
          {
            /* Lucide has no filled circle-X glyph (it's a pure
            outline/stroke icon set) — composed by hand: a solid circle
            with a smaller X cut into it in the background color, the
            familiar "clear field" shape (iOS/macOS search fields) rather
            than a bare X that reads more like "close" than "clear". */
          }
          {query.length > 0 && (
            <Pressable
              onPress={() => {
                setQuery("");
                setResults([]);
              }}
              hitSlop={12}
              accessibilityRole="button"
              accessibilityLabel={t("map.search.clear_aria_label")}
              style={styles.clear}
            >
              <X size={10} strokeWidth={3} color={color.paper} />
            </Pressable>
          )}
        </View>
      </View>

      <ScrollView style={styles.results} contentContainerStyle={styles.resultsRoom} keyboardShouldPersistTaps="handled">
        {query.trim().length > 0 && results.length === 0 && <Text style={styles.empty}>{loading ? t("map.search.loading") : t("map.search.no_results")}</Text>}

        {
          /* Divisions first — the backend already orders them that way
          (search/provider/overture_provider.ts's own top comment: a
          name matching an admin area is rare enough to surface ahead of
          whatever POI happens to contain the same substring), and a
          section header on each group makes that ordering legible
          instead of just an unlabeled jump from areas to places. */
        }
        {divisionResults.length > 0 && <SectionHeader label={t("map.search.section_areas")} />}
        {divisionResults.map((r) => (
          <Pressable key={r.gers_id} onPress={() => select(r)} accessibilityRole="button" style={styles.row}>
            {
              /* No icon here, unlike a POI row — a division doesn't
              have a "kind of place" to badge the way a category does,
              and the section header already says what these are. */
            }
            <View style={styles.rowText}>
              <Text numberOfLines={1} style={styles.name}>{displayName(r, lang)}</Text>
              <SubtitleRow label={r.context.length > 0 ? assembleLabel(r.context, lang) : ""} distance={distanceOf(r)} />
            </View>
          </Pressable>
        ))}

        {poiResults.length > 0 && <SectionHeader label={t("map.search.section_places")} />}
        {poiResults.map((r) => {
          // poi-badge.ts says why an emoji beats a Lucide glyph at this size.
          const badge = resolvePoiBadge(r.category);
          return (
            <Pressable key={r.gers_id} onPress={() => select(r)} accessibilityRole="button" style={styles.row}>
              <View style={[styles.badge, { backgroundColor: `${badge.color}33` }]}>
                <Text style={styles.badgeIcon}>{badge.icon}</Text>
              </View>
              {
                /* Grows, or the column sizes to its own text and the
                distance ends up beside the address instead of at the
                row's right edge. */
              }
              <View style={styles.rowText}>
                <Text numberOfLines={1} style={styles.name}>{displayName(r, lang)}</Text>
                <SubtitleRow label={r.context.length > 0 ? assembleLabel(r.context, lang) : ""} distance={distanceOf(r)} />
              </View>
            </Pressable>
          );
        })}
      </ScrollView>

      <LocateCard />
    </Panel>
  );
}

const styles = StyleSheet.create({
  header: { borderBottomWidth: 1, borderBottomColor: alpha(color.text, 0.06) },
  locateCard: { borderRadius: 22, pointerEvents: "auto", boxShadow: "0 6px 20px rgba(0, 0, 0, 0.12)" },
  locateRow: { height: 44, flexDirection: "row", alignItems: "center", gap: 10, paddingHorizontal: 16 },
  locateLabel: { flex: 1, ...textStyle.body, color: color.text },
  locateAction: { ...textStyle.body, fontWeight: weight.semibold, color: color.chosen },
  // Room under the last result for the card that floats over it.
  resultsRoom: { paddingBottom: 76 },
  headerRow: { height: PANEL_HEADER_PX, flexDirection: "row", alignItems: "center", gap: 10, paddingHorizontal: 16 },
  back: { width: 42, height: 42, marginLeft: -10, borderRadius: 21, alignItems: "center", justifyContent: "center" },
  field: { flex: 1, minWidth: 0, color: color.text },
  clear: { width: 16, height: 16, borderRadius: 8, alignItems: "center", justifyContent: "center", backgroundColor: alpha(color.textMuted, 0.5) },
  results: { flex: 1 },
  empty: { paddingHorizontal: 16, paddingVertical: 12, ...textStyle.footnote, color: color.textMuted },
  sectionHeader: {
    paddingHorizontal: 16,
    paddingTop: 12,
    paddingBottom: 6,
    ...sectionTitle,
    color: alpha(color.textMuted, 0.7),
  },
  row: { flexDirection: "row", alignItems: "center", gap: 10, paddingHorizontal: 16, paddingVertical: 12, borderBottomWidth: 1, borderBottomColor: hairline },
  rowText: { flex: 1, minWidth: 0, gap: 2 },
  name: { ...textStyle.body, color: color.text },
  subtitle: { flexDirection: "row", alignItems: "baseline", gap: 8 },
  small: { ...textStyle.caption, color: color.textMuted },
  context: { flexShrink: 1 },
  distance: { marginLeft: "auto", flexShrink: 0, fontVariant: ["tabular-nums"] },
  badge: { width: 24, height: 24, borderRadius: 12, alignItems: "center", justifyContent: "center", flexShrink: 0 },
  badgeIcon: { fontSize: textStyle.footnote.fontSize },
});
