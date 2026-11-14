// @pxl/client/ui/feature/setting/row.tsx

import type { ReactNode } from "react";
import { Linking, Pressable, StyleSheet, Text, View } from "react-native";

import type { GlyphName } from "@/ui/shared/component/icon.tsx";
import { ToggleRow } from "@/ui/feature/setting/toggle-row.tsx";

import type { SettingCost } from "@/ui/shared/setting.ts";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import {
  alpha,
  chrome,
  color,
  hairline,
  text_scale,
  text_style,
  weight,
} from "@/ui/shared/component/theme.ts";
import { SectionTitle } from "@/ui/shared/component/section-title.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";

const INSET = 12;
const GROUP_BORDER = 1;

const COST_COLOR: Record<SettingCost, string> = {
  low: color.cost_low,
  med: color.cost_medium,
  high: color.cost_high,
};

function Badge({ text, tint }: { text: string; tint: string }) {
  return (
    <View style={[styles.badge, { backgroundColor: alpha(tint, 0.15) }]}>
      <Text
        maxFontSizeMultiplier={text_scale.mark}
        style={[styles.badge_text, { color: tint }]}
      >
        {text}
      </Text>
    </View>
  );
}

function Badges({ cost, dev }: { cost?: SettingCost; dev?: boolean }) {
  return (
    <>
      {dev && <Badge text="DEV" tint={color.dev} />}
      {cost && <Badge text={cost.toUpperCase()} tint={COST_COLOR[cost]} />}
    </>
  );
}

function Described({
  label,
  description,
  cost,
  dev,
}: {
  label: string;
  description?: string;
  cost?: SettingCost;
  dev?: boolean;
}) {
  return (
    <View style={styles.described}>
      <View style={styles.name_line}>
        <Text style={styles.name}>{label}</Text>
        <Badges cost={cost} dev={dev} />
      </View>
      {description && <Text style={setting_text.small}>{description}</Text>}
    </View>
  );
}

interface SettingRowProps {
  label: string;
  description: string;
  active: boolean;
  /** Takes the new value rather than a flip, because a flip would depend on
   * the current value having already arrived from the core. */
  onChange: (active: boolean) => void;
  cost?: SettingCost;
  dev?: boolean;
}

export function SettingRow(props: SettingRowProps) {
  const { label, description, active, onChange, cost, dev } = props;
  return (
    <ToggleRow
      value={active}
      onValueChange={onChange}
      style={[styles.row, styles.bleed]}
    >
      <Described
        label={label}
        description={description}
        cost={cost}
        dev={dev}
      />
    </ToggleRow>
  );
}

interface SettingGroupProps {
  label: string;
  description?: string;
  cost?: SettingCost;
  dev?: boolean;
  children: ReactNode;
}

export function SettingGroup(props: SettingGroupProps) {
  const { label, description, cost, dev, children } = props;
  return (
    <View
      accessibilityRole="radiogroup"
      accessibilityLabel={label}
      style={[styles.group, styles.bleed]}
    >
      <View style={styles.legend}>
        <Text
          maxFontSizeMultiplier={text_scale.chrome}
          style={styles.legend_text}
        >
          {label}
        </Text>
        <Badges cost={cost} dev={dev} />
      </View>
      {description && (
        <Text style={[setting_text.small, styles.group_description]}>
          {description}
        </Text>
      )}
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
  dev?: boolean;
}

/** The check's slot keeps its width when empty, so that selecting a row does
 * not rewrap its text. */
export function SettingChoiceRow(props: SettingChoiceRowProps) {
  const { label, description, selected, onSelect, cost, dev } = props;
  return (
    <Pressable
      onPress={onSelect}
      accessibilityRole="radio"
      aria-checked={selected}
      style={styles.row}
    >
      <Described
        label={label}
        description={description}
        cost={cost}
        dev={dev}
      />
      <View style={styles.check}>
        {selected && (
          <Glyph
            glyph="checkmark"
            size={chrome.glyph}
            weight="semibold"
            color={color.chosen}
          />
        )}
      </View>
    </Pressable>
  );
}

export function SettingChoices({ children }: { children: ReactNode }) {
  return <View style={styles.bleed}>{children}</View>;
}

interface SectionProps {
  title: string;
  icon?: GlyphName;
  caption?: string;
  children: ReactNode;
}

export function Section(props: SectionProps) {
  const { title, icon, caption, children } = props;
  return (
    <View style={styles.section}>
      <View style={styles.title_block}>
        <View style={styles.title_line}>
          {icon && <Glyph glyph={icon} size={16} color={LABEL.secondary} />}
          <SectionTitle>{title}</SectionTitle>
        </View>
        {caption && <Text style={setting_text.faint}>{caption}</Text>}
      </View>
      {children}
    </View>
  );
}

export function Prose({ children }: { children: ReactNode }) {
  return <View style={styles.prose}>{children}</View>;
}

export function Link({
  href,
  children,
}: {
  href: string;
  children?: ReactNode;
}) {
  return (
    <Text style={styles.link} onPress={() => Linking.openURL(href)}>
      {children}
    </Text>
  );
}

export const setting_text = StyleSheet.create({
  small: { ...text_style.caption, color: LABEL.secondary },
  faint: {
    ...text_style.caption2,
    fontWeight: weight.regular,
    color: LABEL.secondary,
  },
});

const styles = StyleSheet.create({
  row: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 12,
    paddingHorizontal: INSET,
    paddingVertical: 12,
    borderRadius: 8,
  },
  bleed: { marginHorizontal: -INSET },
  described: { flexShrink: 1, minWidth: 0, gap: 4 },
  name_line: { flexDirection: "row", alignItems: "center", gap: 8 },
  name: { ...text_style.body, color: LABEL.primary },

  badge: {
    height: 16,
    justifyContent: "center",
    borderRadius: 8,
    paddingHorizontal: 4,
  },
  badge_text: {
    ...text_style.caption2,
    fontSize: 9,
    lineHeight: 10,
    letterSpacing: 0.4,
  },

  group: {
    marginTop: 16,
    marginBottom: 12,
    borderWidth: GROUP_BORDER,
    borderColor: hairline,
    borderRadius: 12,
    padding: 4,
    paddingTop: 12,
  },
  // Its top on the border's middle and raised by half its own height, so that
  // it centers on the border at any text size.
  legend: {
    position: "absolute",
    top: -GROUP_BORDER / 2,
    left: 8,
    transform: [{ translateY: "-50%" }],
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    paddingHorizontal: 8,
    backgroundColor: color.ground,
  },
  legend_text: {
    ...text_style.caption,
    fontWeight: weight.medium,
    color: LABEL.primary,
  },
  group_description: {
    paddingHorizontal: INSET,
    paddingTop: 4,
    paddingBottom: 4,
  },
  answers: { gap: 4 },

  check: { width: chrome.glyph, height: chrome.glyph, flexShrink: 0 },

  section: { gap: 4 },
  prose: { gap: 8, paddingVertical: 4 },
  link: { textDecorationLine: "underline" },
  title_block: { gap: 4, marginBottom: 4 },
  title_line: { flexDirection: "row", alignItems: "center", gap: 8 },
});
