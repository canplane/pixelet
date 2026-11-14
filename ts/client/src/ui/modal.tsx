// @pxl/client/ui/modal.tsx

import { useState } from "react";

import { PanelPresence } from "@/ui/shared/component/panel.tsx";
import { close_modal, type Modal, useAskedModal } from "@/ui/shared/modal.ts";

import { SettingsModal } from "@/ui/feature/setting/modal.tsx";
import { DrawerModal } from "@/ui/feature/drawer/modal.tsx";
import { SearchModal } from "@/ui/feature/search/modal.tsx";

export function ModalHost() {
  const asked = useAskedModal();
  // A closed modal stays drawn until its panel has slid away and called
  // `on_exited`.
  const [shown, set_shown] = useState<Modal | null>(null);
  if (asked !== null && asked !== shown) set_shown(asked);
  if (shown === null) return null;

  const on_close = close_modal;
  return (
    <PanelPresence.Provider
      value={{ open: asked !== null, on_exited: () => set_shown(null) }}
    >
      {shown === "settings" && <SettingsModal onClose={on_close} />}
      {shown === "drawer" && <DrawerModal onClose={on_close} />}
      {shown === "search" && <SearchModal onClose={on_close} />}
    </PanelPresence.Provider>
  );
}
