// @pxl/web/app/settings/settings_modal.tsx

import { useTranslation } from "react-i18next";

import type { BaseModalProps } from "@/app/types/ui.types.ts";
import { X } from "lucide-react";

import { Sheet, SheetClose, SheetContent, SheetDescription, SheetTitle } from "@/shared/ui/theme/base/sheet.tsx";
import { Separator } from "@/shared/ui/theme/base/separator.tsx";
import { LanguageSection } from "./sections/language_section.tsx";
import { LightingSection } from "./sections/lighting_section.tsx";
import { PostFxSection } from "./sections/post_fx_section.tsx";
import { QualitySection } from "./sections/quality_section.tsx";
import { DebugSection } from "./sections/debug_section.tsx";
import { DisclaimerSection } from "./sections/disclaimer_section.tsx";
import { AboutSection } from "./sections/about_section.tsx";
import { AttributionSection } from "./sections/attribution_section.tsx";

export type SettingsModalProps = {
  type: "settings";
};

export function SettingsModal(props: BaseModalProps & SettingsModalProps) {
  const { open, on_open_change } = props;
  const { t } = useTranslation();

  return (
    <Sheet open={open} onOpenChange={on_open_change}>
      <SheetContent
        side="left"
        hideClose
        className="sm:w-[360px] p-0 gap-0 flex flex-col"
        overlayClassName="bg-transparent"
        overlayStyle={{ pointerEvents: "none" }}
      >
        <div className="shrink-0 border-b border-border/60" style={{ paddingTop: "env(safe-area-inset-top, 0px)" }}>
          <div className="flex h-14 items-center gap-2 px-5">
            <SheetTitle className="flex-1 text-[17px] font-semibold">{t("settings.title")}</SheetTitle>
            <SheetDescription className="sr-only">{t("settings.description")}</SheetDescription>
            <SheetClose
              className="-mr-2.5 flex size-[42px] items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer"
              aria-label={t("settings.close")}
            >
              <X className="size-5" />
            </SheetClose>
          </div>
        </div>

        <div
          className="flex-1 overflow-y-auto overscroll-contain flex flex-col gap-4 px-5 pt-4"
          style={{ paddingBottom: "calc(env(safe-area-inset-bottom, 0px) + 1rem)" }}
        >
          <p className="text-[11px] leading-snug text-muted-foreground/70">{t("settings.cost_legend")}</p>

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
          <LanguageSection />

          <Separator />
          <DisclaimerSection />
          <AttributionSection />
          <AboutSection />
        </div>
      </SheetContent>
    </Sheet>
  );
}
