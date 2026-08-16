// @pxl/web/app/settings/settings_modal.tsx

import { useTranslation } from "react-i18next";

import type { BaseModalProps } from "@/app/types/ui.types.ts";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/shared/ui/theme/base/sheet.tsx";
import { Separator } from "@/shared/ui/theme/base/separator.tsx";
import { LanguageSection } from "./sections/language_section.tsx";
import { LightingSection } from "./sections/lighting_section.tsx";
import { PostFxSection } from "./sections/post_fx_section.tsx";
import { QualitySection } from "./sections/quality_section.tsx";
import { DebugSection } from "./sections/debug_section.tsx";
import { LicenseSection } from "./sections/license_section.tsx";

/** SettingsModalProps */
export type SettingsModalProps = {
  type: "settings";
};

/**
 * Settings modal — language, rendering-effect toggles (always reachable,
 * real graphics options, split by category into ./sections/*), dev-only
 * diagnostics (__IS_DEV__-gated), and the required OSM/Copernicus
 * data-source attribution (was previously collapsed behind a map-corner ⓘ
 * in bottom_overlays.tsx — see that file's own removed comment for why this
 * is its real home now).
 */
export function SettingsModal(props: BaseModalProps & SettingsModalProps) {
  const { open, on_open_change } = props;
  const { t } = useTranslation();

  return (
    <Sheet open={open} onOpenChange={on_open_change}>
      <SheetContent
        side="left"
        className="w-[min(360px,100vw)] overflow-y-auto"
        overlayClassName="bg-transparent"
        overlayStyle={{ pointerEvents: "none" }}
      >
        <SheetHeader className="mb-3">
          <SheetTitle>{t("settings.title")}</SheetTitle>
          <SheetDescription className="sr-only">{t("settings.description")}</SheetDescription>
        </SheetHeader>

        <div className="flex flex-col gap-4 px-4">
          <LanguageSection />
          <Separator />
          <LightingSection />
          <PostFxSection />
          <QualitySection />

          {__IS_DEV__ && (
            <>
              <Separator />
              <DebugSection />
            </>
          )}

          <Separator />
          <LicenseSection />
        </div>
      </SheetContent>
    </Sheet>
  );
}
