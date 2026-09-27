// @pxl/web/feature/setting/row.tsx

import type { ReactNode } from "react";
import { Pressable, StyleSheet, Text, View } from "react-native";

import type { GlyphName } from "@platform/icon.tsx";
import { ToggleRow } from "@platform/toggle-row.tsx";

import type { SettingCost } from "@/feature/setting/schema.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { alpha, chrome, color, hairline, sectionTitle, textStyle, weight } from "@/shared/ui/theme.ts";
import { LABEL } from "@platform/palette.ts";

const COST_COLOR: Record<SettingCost, string> = {
  low: color.costLow,
  med: color.costMedium,
  high: color.costHigh,
};

function Badge({ text, tint }: { text: string; tint: string }) {
  return (
    <View style={[styles.badge, { backgroundColor: alpha(tint, 0.15) }]}>
      <Text style={[styles.badgeText, { color: tint }]}>{text}</Text>
    </View>
  );
}

/** A setting's badges: that only a development build offers it, and what it
 * costs the device. */
function Badges({ cost, dev }: { cost?: SettingCost; dev?: boolean }) {
  return (
    <>
      {dev && <Badge text="DEV" tint={color.dev} />}
      {cost && <Badge text={cost.toUpperCase()} tint={COST_COLOR[cost]} />}
    </>
  );
}

/** A name, its badges, and an optional description under it. */
function Described({ label, description, cost, dev }: { label: string; description?: string; cost?: SettingCost; dev?: boolean }) {
  return (
    <View style={styles.described}>
      <View style={styles.nameLine}>
        <Text style={styles.name}>{label}</Text>
        <Badges cost={cost} dev={dev} />
      </View>
      {description && <Text style={settingText.small}>{description}</Text>}
    </View>
  );
}

interface SettingRowProps {
  label: string;
  description: string;
  active: boolean;
  /** Called with the switch's new value, which is sent to the kernel as is.
   * Sending a flip instead would depend on the current value having arrived
   * already. */
  onChange: (active: boolean) => void;
  cost?: SettingCost;
  /** Marks a setting only a development build offers (schema.json's `dev`) — the table decides whether the row is there at all, this just labels it once shown. */
  dev?: boolean;
}

/** A toggle: the platform's row for a switch (platform toggle-row.tsx). */
export function SettingRow(props: SettingRowProps) {
  const { label, description, active, onChange, cost, dev } = props;
  return (
    <ToggleRow value={active} onValueChange={onChange} style={[styles.row, styles.bleed]}>
      <Described label={label} description={description} cost={cost} dev={dev} />
    </ToggleRow>
  );
}

interface SettingGroupProps {
  label: string;
  cost?: SettingCost;
  dev?: boolean;
  children: ReactNode;
}

/** A choice's answers in one box, the setting's name set into its border as a
 * fieldset's legend: on the panel's ground, so the line seems to run behind
 * it, and at the left, where the answers' names start. The box stands apart
 * from the rows around it so it reads as a box and not another row, more
 * above than below, since the name rises out of it by half its height. */
export function SettingGroup(props: SettingGroupProps) {
  const { label, cost, dev, children } = props;
  return (
    <View accessibilityRole="radiogroup" accessibilityLabel={label} style={[styles.group, styles.bleed]}>
      <View style={styles.legend}>
        <Text style={styles.legendText}>{label}</Text>
        <Badges cost={cost} dev={dev} />
      </View>
      <View style={styles.answers}>{children}</View>
    </View>
  );
}

interface SettingChoiceRowProps {
  label: string;
  description?: string;
  selected: boolean;
  onSelect: () => void;
  cost?: SettingCost;
}

/** One option of a single choice (a setting's options, the language list).
 * Same layout as SettingRow; the selected option shows a check, the others
 * nothing. The check slot keeps its width when empty so selecting does not
 * rewrap the text. Unlike SettingRow it does not reach into the panel's
 * padding: its group box or list handles that. */
export function SettingChoiceRow(props: SettingChoiceRowProps) {
  const { label, description, selected, onSelect, cost } = props;
  return (
    <Pressable onPress={onSelect} accessibilityRole="radio" aria-checked={selected} style={styles.row}>
      <Described label={label} description={description} cost={cost} />
      <View style={styles.check}>{selected && <Glyph glyph="checkmark" size={chrome.glyph} weight="semibold" color={color.chosen} />}</View>
    </Pressable>
  );
}

interface SectionTitleProps {
  children: string;
  icon?: GlyphName;
  caption?: string;
}

export function SectionTitle(props: SectionTitleProps) {
  const { children, icon, caption } = props;
  return (
    <View style={styles.titleBlock}>
      <View style={styles.titleLine}>
        {icon && <Glyph glyph={icon} size={16} color={LABEL.secondary} />}
        <Text accessibilityRole="header" style={styles.title}>{children}</Text>
      </View>
      {caption && <Text style={settingText.faint}>{caption}</Text>}
    </View>
  );
}

/** Settings' shared type: the panel's small print. */
export const settingText = StyleSheet.create({
  small: { ...textStyle.caption, color: LABEL.secondary },
  faint: { ...textStyle.caption2, fontWeight: weight.regular, color: LABEL.secondary },
  link: { textDecorationLine: "underline" },
});

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 12, paddingHorizontal: 12, paddingVertical: 12, borderRadius: 8 },
  // A row reaches into the panel's padding by its own, so its words line up
  // with the section title's.
  bleed: { marginHorizontal: -12 },
  described: { flexShrink: 1, minWidth: 0, gap: 4 },
  nameLine: { flexDirection: "row", alignItems: "center", gap: 8 },
  name: { ...textStyle.body, color: LABEL.primary },

  // iOS's tinted style: the tint as a faint ground and the words in it, no
  // edge — a full fill is what a chosen chip says.
  badge: { height: 16, justifyContent: "center", borderRadius: 8, paddingHorizontal: 4 },
  // Under the smallest style on purpose: a badge is a tag, read as a shape at
  // a glance and not as text, the size iOS's own tab labels and an app's
  // counters go down to. Its capitals, weight and color carry it. The badge
  // stands 16 tall, Material's badge height.
  badgeText: { ...textStyle.caption2, fontSize: 9, lineHeight: 10, letterSpacing: 0.4 },

  group: { marginTop: 16, marginBottom: 12, borderWidth: 1, borderColor: hairline, borderRadius: 12, padding: 4, paddingTop: 12 },
  legend: {
    position: "absolute",
    // The name's middle on the box's one-point border.
    top: 1 - textStyle.caption.lineHeight / 2,
    left: 8,
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    paddingHorizontal: 8,
    backgroundColor: color.ground,
  },
  legendText: { ...textStyle.caption, fontWeight: weight.medium, color: LABEL.primary },
  answers: { gap: 4 },

  check: { width: chrome.glyph, height: chrome.glyph, flexShrink: 0 },

  titleBlock: { gap: 4, marginBottom: 4 },
  titleLine: { flexDirection: "row", alignItems: "center", gap: 8 },
  title: { ...sectionTitle, color: LABEL.secondary },
});
