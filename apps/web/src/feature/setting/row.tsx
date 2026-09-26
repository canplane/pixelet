// @pxl/web/feature/setting/row.tsx

import type { LucideIcon } from "lucide-react-native";
import type { ReactNode } from "react";
import { Pressable, StyleSheet, Switch, Text, View } from "react-native";

import type { SettingCost } from "@/feature/setting/schema.ts";
import { alpha, color, font, hairline } from "@/shared/ui/theme.ts";

const COST_COLOR: Record<SettingCost, string> = {
  low: color.costLow,
  med: color.costMedium,
  high: color.costHigh,
};

function Badge({ text, tint }: { text: string; tint: string }) {
  return (
    <View style={[styles.badge, { borderColor: alpha(tint, 0.3), backgroundColor: alpha(tint, 0.1) }]}>
      <Text style={[styles.badgeText, { color: tint }]}>{text}</Text>
    </View>
  );
}

function CostBadge(props: { cost: SettingCost }) {
  return <Badge text={props.cost.toUpperCase()} tint={COST_COLOR[props.cost]} />;
}

function DevBadge() {
  return <Badge text="DEV" tint={color.dev} />;
}

/** A name, its badges, and what it does under it. */
function Described({ label, description, cost, dev }: { label: string; description: string; cost?: SettingCost; dev?: boolean }) {
  return (
    <View style={styles.described}>
      <View style={styles.nameLine}>
        <Text style={styles.name}>{label}</Text>
        {dev && <DevBadge />}
        {cost && <CostBadge cost={cost} />}
      </View>
      <Text style={styles.description}>{description}</Text>
    </View>
  );
}

interface SettingRowProps {
  label: string;
  description: string;
  active: boolean;
  onToggle: () => void;
  cost?: SettingCost;
  /** Marks a setting only a development build offers (schema.json's `dev`) — the table decides whether the row is there at all, this just labels it once shown. */
  dev?: boolean;
}

/** A toggle: the whole row is its switch. */
export function SettingRow(props: SettingRowProps) {
  const { label, description, active, onToggle, cost, dev } = props;
  return (
    <Pressable onPress={onToggle} accessibilityRole="switch" aria-checked={active} style={[styles.row, styles.bleed]}>
      <Described label={label} description={description} cost={cost} dev={dev} />
      <Switch
        value={active}
        onValueChange={onToggle}
        trackColor={{ false: color.track, true: color.chosen }}
        thumbColor={color.white}
        {
          // react-native-web colors an on thumb by this alone, and teal where
          // it is not given; React Native's types do not name it.
          ...{ activeThumbColor: color.white }
        }
      />
    </Pressable>
  );
}

interface SettingGroupProps {
  label: string;
  cost?: SettingCost;
  dev?: boolean;
  children: ReactNode;
}

/** A choice's answers in one box, with the setting's name let into the line
 * that draws it — a fieldset's legend, drawn rather than declared. The
 * answers each name an answer and nothing said what they were answers to; a
 * name floating above the box would have been a second thing to place, so
 * the name sits on the border itself, carrying the panel's own background
 * so the line appears to run through it on either side. The box takes the
 * top padding the name needs to clear the first answer, and the name is
 * inset to the left rather than centered, since every other line in the
 * panel starts at the left — by its own padding, so the name starts exactly
 * where the answers' names start.
 *
 * The box stands away from the rows it sits among — a switch's row has only
 * the section's own gap around it, which left this one reading as another
 * row rather than as a box of answers. More above than below, because the
 * name rises out of the box by half its height and would otherwise close on
 * whatever is over it. */
export function SettingGroup(props: SettingGroupProps) {
  const { label, cost, dev, children } = props;
  return (
    <View accessibilityRole="radiogroup" accessibilityLabel={label} style={[styles.group, styles.bleed]}>
      <View style={styles.legend}>
        <Text style={styles.legendText}>{label}</Text>
        {dev && <DevBadge />}
        {cost && <CostBadge cost={cost} />}
      </View>
      <View style={styles.answers}>{children}</View>
    </View>
  );
}

interface SettingRadioRowProps {
  label: string;
  description: string;
  selected: boolean;
  onSelect: () => void;
  cost?: SettingCost;
}

/** One answer of a choice — the same layout as SettingRow, with a radio
 * mark, since picking one answer here drops the others. It sits inside its
 * group's box, so it does not bleed to the panel's edge as a row does. */
export function SettingRadioRow(props: SettingRadioRowProps) {
  const { label, description, selected, onSelect, cost } = props;
  return (
    <Pressable onPress={onSelect} accessibilityRole="radio" aria-checked={selected} style={styles.row}>
      <Described label={label} description={description} cost={cost} />
      <View style={[styles.radio, selected && styles.radioOn]}>{selected && <View style={styles.radioDot} />}</View>
    </Pressable>
  );
}

interface SectionTitleProps {
  children: string;
  icon?: LucideIcon;
  caption?: string;
  /** Marks the whole section as a development build's (schema.json's
   * DEV_SECTIONS) — section.tsx decides whether it is shown at all, this just
   * labels it once shown. */
  dev?: boolean;
}

export function SectionTitle(props: SectionTitleProps) {
  const { children, icon: Icon, caption, dev } = props;
  return (
    <View style={styles.titleBlock}>
      <View style={styles.titleLine}>
        {Icon && <Icon size={16} color={color.textMuted} />}
        <Text accessibilityRole="header" style={styles.title}>{children}</Text>
        {dev && <DevBadge />}
      </View>
      {caption && <Text style={styles.caption}>{caption}</Text>}
    </View>
  );
}

/** Settings' shared type: the panel's small print. */
export const settingText = StyleSheet.create({
  small: { fontFamily: font.family, fontSize: 12, lineHeight: 17, color: color.textMuted },
  faint: { fontFamily: font.family, fontSize: 11, lineHeight: 15, color: alpha(color.textMuted, 0.7) },
  link: { textDecorationLine: "underline" },
});

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 12, paddingHorizontal: 12, paddingVertical: 10, borderRadius: 8 },
  // A row reaches past the panel's padding to its edge, so its own padding
  // lines its words up with the section title's.
  bleed: { marginHorizontal: -12 },
  described: { flexShrink: 1, minWidth: 0, gap: 4 },
  nameLine: { flexDirection: "row", alignItems: "center", gap: 6 },
  name: { fontFamily: font.family, fontSize: 15, fontWeight: "500", lineHeight: 19, color: color.text },
  description: settingTextOf(12, color.textMuted),

  badge: { borderWidth: 1, borderRadius: 999, paddingHorizontal: 4, paddingVertical: 2 },
  badgeText: { fontFamily: font.family, fontSize: 9, fontWeight: "600", lineHeight: 10, letterSpacing: 0.4 },

  group: { marginTop: 16, marginBottom: 10, borderWidth: 1, borderColor: hairline, borderRadius: 10, padding: 4, paddingTop: 10 },
  legend: {
    position: "absolute",
    top: -7,
    left: 8,
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    paddingHorizontal: 8,
    backgroundColor: color.paper,
  },
  legendText: { fontFamily: font.family, fontSize: 12, fontWeight: "500", lineHeight: 13, color: color.text },
  answers: { gap: 2 },

  radio: { width: 16, height: 16, borderRadius: 8, borderWidth: 1.5, borderColor: color.track, alignItems: "center", justifyContent: "center", flexShrink: 0 },
  radioOn: { borderColor: color.chosen },
  radioDot: { width: 8, height: 8, borderRadius: 4, backgroundColor: color.chosen },

  titleBlock: { gap: 4, marginBottom: 4 },
  titleLine: { flexDirection: "row", alignItems: "center", gap: 6 },
  title: { fontFamily: font.family, fontSize: 12, fontWeight: "600", letterSpacing: 0.6, textTransform: "uppercase", color: color.textMuted },
  caption: settingTextOf(11, alpha(color.textMuted, 0.7)),
});

function settingTextOf(fontSize: number, tint: string) {
  return { fontFamily: font.family, fontSize, lineHeight: Math.round(fontSize * 1.4), color: tint };
}
