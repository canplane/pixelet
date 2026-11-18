// @pxl/client/ui/feature/setting/info/licenses.tsx

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { SETTING_TEXT } from "@/ui/feature/setting/row.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";
import { WEIGHT } from "@/ui/shared/component/theme.ts";
import LICENSES from "./licenses.json" with { type: "json" };

/**
 * The software the app is built from that someone else wrote, each package
 * with its version, its license and the license's text, as script/licenses.ts
 * gathers them from what the app holds, the last of Attribution's groups.
 * The list opens on a touch, and a package's text on a touch of the package.
 * Not translated.
 */
export function Licenses() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [shown, setShown] = useState<number | null>(null);
  const count = LICENSES.packages.length;
  return (
    <View style={STYLE.licenses}>
      <Text style={STYLE.toggle} onPress={() => setOpen(!open)}>
        {open
          ? t("settings.attribution.hide")
          : t("settings.attribution.show", { count })}
      </Text>
      {open && (
        <View style={STYLE.list}>
          {LICENSES.packages.map((p, i) => (
            <View key={`${p.name}@${p.version}`} style={STYLE.item}>
              <Text
                style={SETTING_TEXT.small}
                onPress={() => setShown(shown === i ? null : i)}
              >
                <Text style={STYLE.name}>{p.name}</Text> {p.version} ·{" "}
                {p.license}
              </Text>
              {shown === i && (
                <Text style={STYLE.text}>{LICENSES.texts[p.text]}</Text>
              )}
            </View>
          ))}
        </View>
      )}
    </View>
  );
}

const STYLE = StyleSheet.create({
  licenses: { gap: 12 },
  toggle: { ...SETTING_TEXT.small, textDecorationLine: "underline" },
  list: { gap: 8 },
  item: { gap: 4 },
  name: { fontWeight: WEIGHT.medium, color: LABEL.primary },
  text: SETTING_TEXT.faint,
});
