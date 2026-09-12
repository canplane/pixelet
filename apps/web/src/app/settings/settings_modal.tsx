// @pxl/web/app/settings/settings_modal.tsx

import { useTranslation } from "react-i18next";

import type { BaseModalProps } from "@/app/types/ui.types.ts";
import { Info, Sparkles, X } from "lucide-react";

import { Sheet, SheetClose, SheetContent, SheetDescription, SheetTitle } from "@/shared/ui/shadcn/sheet.tsx";
import { Separator } from "@/shared/ui/shadcn/separator.tsx";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/shared/ui/shadcn/tabs.tsx";
import { SettingsSection } from "./settings_sections.tsx";
import { LanguageSection } from "./sections/language_section.tsx";
import { DebugSection } from "./sections/debug_section.tsx";
import { DisclaimerSection } from "./sections/disclaimer_section.tsx";
import { AboutSection } from "./sections/about_section.tsx";
import { AttributionSection } from "./sections/attribution_section.tsx";

export type SettingsModalProps = {
  type: "settings";
};

export function SettingsModal(props: BaseModalProps & SettingsModalProps) {
  const { open, onOpenChange } = props;
  const { t } = useTranslation();

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="left"
        hideClose
        className="sm:w-[360px] p-0 gap-0 flex flex-col bg-background text-foreground"
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

        <Tabs defaultValue="effects" className="flex-1 min-h-0">
          <TabsList className="mx-5 mt-4">
            <TabsTrigger value="effects">
              <Sparkles className="size-3.5" />
              {t("settings.tabs.effects")}
            </TabsTrigger>
            <TabsTrigger value="info">
              <Info className="size-3.5" />
              {t("settings.tabs.info")}
            </TabsTrigger>
          </TabsList>

          <div
            className="flex-1 overflow-y-auto overscroll-contain px-5 pt-4"
            style={{ paddingBottom: "calc(env(safe-area-inset-bottom, 0px) + 1rem)" }}
          >
            <TabsContent value="effects">
              <p className="text-[11px] leading-snug text-muted-foreground/70">{t("settings.cost_legend")}</p>
              <Separator />
              <SettingsSection section="display" />
              <SettingsSection section="camera" />
              <SettingsSection section="editing" />
              <SettingsSection section="lighting" />
              <SettingsSection section="quality" />
              {__IS_DEV__ && (
                <>
                  <Separator />
                  <DebugSection />
                </>
              )}
            </TabsContent>

            <TabsContent value="info">
              <LanguageSection />
              <Separator />
              <DisclaimerSection />
              <AttributionSection />
              <AboutSection />
            </TabsContent>
          </div>
        </Tabs>
      </SheetContent>
    </Sheet>
  );
}
