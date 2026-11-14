// @pxl/client/ui/feature/setting/row.tsx

import type { ReactNode } from "react";
import { Linking, Pressable, StyleSheet, Text, View } from "react-native";

import type { GlyphName } from "@/ui/shared/component/icon.tsx";
import { ToggleRow } from "@/ui/feature/setting/toggle-row.tsx";

import type { SettingCost } from "@/ui/shared/setting.ts";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import {
  alpha,
  CHROME,
  COLOR,
  HAIRLINE,
  TEXT_SCALE_CAP,
  TEXT_STYLE,
  WEIGHT,
} from "@/ui/shared/component/theme.ts";
import { SectionTitle } from "@/ui/shared/component/section-title.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";

const INSET = 12;
const GROUP_BORDER = 1;

const COST_COLOR: Record<SettingCost, string> = {
  low: COLOR.cost_low,
  med: COLOR.cost_medium,
  high: COLOR.cost_high,
};

function Badge({ text, tint }: { text: string; tint: string }) {
  return (
    <View style={[STYLE.badge, { backgroundColor: alpha(tint, 0.15) }]}>
      <Text
        maxFontSizeMultiplier={TEXT_SCALE_CAP.mark}
        style={[STYLE.badge_text, { color: tint }]}
      >
        {text}
      </Text>
    </View>
  );
}

function Badges({ cost, dev }: { cost?: SettingCost; dev?: boolean }) {
  return (
    <>
      {dev && <Badge text="DEV" tint={COLOR.dev} />}
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
    <View style={STYLE.described}>
      <View style={STYLE.name_line}>
        <Text style={STYLE.name}>{label}</Text>
        <Badges cost={cost} dev={dev} />
      </View>
      {description && <Text style={SETTING_TEXT.small}>{description}</Text>}
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
      style={[STYLE.row, STYLE.bleed]}
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
      style={[STYLE.group, STYLE.bleed]}
    >
      <View style={STYLE.legend}>
        <Text
          maxFontSizeMultiplier={TEXT_SCALE_CAP.chrome}
          style={STYLE.legend_text}
        >
          {label}
        </Text>
        <Badges cost={cost} dev={dev} />
      </View>
      {description && (
        <Text style={[SETTING_TEXT.small, STYLE.group_description]}>
          {description}
        </Text>
      )}
      <View style={STYLE.answers}>{children}</View>
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
      style={STYLE.row}
    >
      <Described
        label={label}
        description={description}
        cost={cost}
        dev={dev}
      />
      <View style={STYLE.check}>
        {selected && (
          <Glyph
            glyph="checkmark"
            size={CHROME.glyph}
            weight="semibold"
            color={COLOR.chosen}
          />
        )}
      </View>
    </Pressable>
  );
}

export function SettingChoices({ children }: { children: ReactNode }) {
  return <View style={STYLE.bleed}>{children}</View>;
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
    <View style={STYLE.section}>
      <View style={STYLE.title_block}>
        <View style={STYLE.title_line}>
          {icon && <Glyph glyph={icon} size={16} color={LABEL.secondary} />}
          <SectionTitle>{title}</SectionTitle>
        </View>
        {caption && <Text style={SETTING_TEXT.faint}>{caption}</Text>}
      </View>
      {children}
    </View>
  );
}

export function Prose({ children }: { children: ReactNode }) {
  return <View style={STYLE.prose}>{children}</View>;
}

export function Link({
  href,
  children,
}: {
  href: string;
  children?: ReactNode;
}) {
  return (
    <Text style={STYLE.link} onPress={() => Linking.openURL(href)}>
      {children}
    </Text>
  );
}

export const SETTING_TEXT = StyleSheet.create({
  small: { ...TEXT_STYLE.caption, color: LABEL.secondary },
  faint: {
    ...TEXT_STYLE.caption2,
    fontWeight: WEIGHT.regular,
    color: LABEL.secondary,
  },
});

const STYLE = StyleSheet.create({
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
  name: { ...TEXT_STYLE.body, color: LABEL.primary },

  badge: {
    height: 16,
    justifyContent: "center",
    borderRadius: 8,
    paddingHorizontal: 4,
  },
  badge_text: {
    ...TEXT_STYLE.caption2,
    fontSize: 9,
    lineHeight: 10,
    letterSpacing: 0.4,
  },

  group: {
    marginTop: 16,
    marginBottom: 12,
    borderWidth: GROUP_BORDER,
    borderColor: HAIRLINE,
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
    backgroundColor: COLOR.ground,
  },
  legend_text: {
    ...TEXT_STYLE.caption,
    fontWeight: WEIGHT.medium,
    color: LABEL.primary,
  },
  group_description: {
    paddingHorizontal: INSET,
    paddingTop: 4,
    paddingBottom: 4,
  },
  answers: { gap: 4 },

  check: { width: CHROME.glyph, height: CHROME.glyph, flexShrink: 0 },

  section: { gap: 4 },
  prose: { gap: 8, paddingVertical: 4 },
  link: { textDecorationLine: "underline" },
  title_block: { gap: 4, marginBottom: 4 },
  title_line: { flexDirection: "row", alignItems: "center", gap: 8 },
});
