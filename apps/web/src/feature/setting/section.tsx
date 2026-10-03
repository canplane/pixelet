// @pxl/web/feature/setting/section.tsx

import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { View } from "react-native";

import type { GlyphName } from "@platform/icon.tsx";

import { type Setting, type SettingName, SETTINGS, type SettingSection, settingsIn } from "@/feature/setting/schema.ts";
import { SectionTitle, SettingChoiceRow, SettingGroup, SettingRow } from "./row.tsx";
import { Command, encode_setting_set } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";

const SECTION_ICONS: Partial<Record<SettingSection, GlyphName>> = {
  display: "eye",
  navigation: "location",
  camera: "move.3d",
  lighting: "sun.max",
  quality: "gauge.medium",
  debug: "ladybug",
};

function devOption(setting: Setting, option: string): boolean {
  return "dev_options" in setting && (setting.dev_options as readonly string[]).includes(option);
}

/** Strings come from `settings.<section>.<key>` and, for a choice, `.<option>` under that. */
export function SettingsSection(props: { section: SettingSection; children?: ReactNode }) {
  const { section, children } = props;
  const { t } = useTranslation();
  const keys = settingsIn(section);
  if (keys.length === 0 && children === undefined) return null;

  return (
    <View style={{ gap: 4 }}>
      <SectionTitle icon={SECTION_ICONS[section]} caption={t(`settings.${section}.section_caption`)}>
        {t(`settings.${section}.section_title`)}
      </SectionTitle>
      {keys.map((key) => <SettingControl key={key} name={key} />)}
      {children}
    </View>
  );
}

function SettingControl(props: { name: SettingName }) {
  const { name } = props;
  const { t } = useTranslation();
  const setting: Setting = SETTINGS[name];
  const { key } = setting;
  const value = useKernel((s) => s.settings[key]);
  const base = `settings.${setting.section}.${name}`;

  switch (setting.kind) {
    case "toggle":
      return (
        <SettingRow
          label={t(`${base}.label`)}
          description={t(`${base}.description`)}
          active={value === 1}
          onChange={(on) => kernel_command(Command.SETTING_SET, encode_setting_set({ key, value: on ? 1 : 0 }))}
          cost={setting.cost}
          dev={setting.dev}
        />
      );
    case "choice":
      return (
        <SettingGroup label={t(`${base}.label`)} cost={setting.cost} dev={setting.dev}>
          {setting.options.map((option, i) => (__IS_DEV__ || !devOption(setting, option)) && (
            <SettingChoiceRow
              key={option}
              label={t(`${base}.${option}.label`)}
              description={t(`${base}.${option}.description`)}
              selected={value === i}
              onSelect={() => kernel_command(Command.SETTING_SET, encode_setting_set({ key, value: i }))}
              cost={setting.costs?.[i] || setting.cost}
              dev={devOption(setting, option)}
            />
          ))}
        </SettingGroup>
      );
    case "index":
      return null;
  }
}
