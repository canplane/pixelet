// @pxl/web/feature/setting/section.tsx

import { useTranslation } from "react-i18next";
import type { GlyphName } from "@platform/icon.tsx";

import { type Setting, type SettingName, SETTINGS, type SettingSection, settingsIn } from "@/feature/setting/schema.ts";
import { Section, SettingChoiceRow, SettingGroup, SettingRow } from "./row.tsx";
import { kernel_setting, useKernel } from "@/kernel/shell.ts";

const SECTION_ICONS: Partial<Record<SettingSection, GlyphName>> = {
  display: "eye",
  navigation: "location",
  camera: "move.3d",
  lighting: "sun.max",
  debug: "ladybug",
};

function devOption(setting: Setting, option: string): boolean {
  return "dev_options" in setting && setting.dev_options.some((o) => o === option);
}

/** Strings come from `settings.<section>.<key>` and, for a choice, `.<option>` under that. */
export function SettingsSection(props: { section: SettingSection }) {
  const { section } = props;
  const { t } = useTranslation();
  const keys = settingsIn(section);
  if (keys.length === 0) return null;

  return (
    <Section title={t(`settings.${section}.section_title`)} icon={SECTION_ICONS[section]} caption={t(`settings.${section}.section_caption`)}>
      {keys.map((key) => <SettingControl key={key} name={key} />)}
    </Section>
  );
}

function SettingControl(props: { name: SettingName }) {
  const { name } = props;
  const { t, i18n } = useTranslation();
  const setting: Setting = SETTINGS[name];
  const { key } = setting;
  const cost = "cost" in setting ? setting.cost : undefined;
  const dev = "dev" in setting && setting.dev;
  const value = useKernel((s) => s.settings[key]);
  const base = `settings.${setting.section}.${name}`;

  switch (setting.kind) {
    case "toggle":
      return (
        <SettingRow
          label={t(`${base}.label`)}
          description={t(`${base}.description`)}
          active={value === 1}
          onChange={(on) => kernel_setting(key, on ? 1 : 0)}
          cost={cost}
          dev={dev}
        />
      );
    case "choice":
      return (
        <SettingGroup label={t(`${base}.label`)} description={i18n.exists(`${base}.description`) ? t(`${base}.description`) : undefined} cost={cost} dev={dev}>
          {setting.options.map((option, i) => {
            const devOnly = devOption(setting, option);
            return (__IS_DEV__ || !devOnly) && (
              <SettingChoiceRow
                key={option}
                label={t(`${base}.${option}.label`)}
                description={t(`${base}.${option}.description`)}
                selected={value === i}
                onSelect={() => kernel_setting(key, i)}
                cost={("costs" in setting ? setting.costs[i] : "") || cost}
                dev={devOnly}
              />
            );
          })}
        </SettingGroup>
      );
    case "index":
      return null;
  }
}
