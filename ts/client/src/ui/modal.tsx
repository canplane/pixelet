// @pxl/client/ui/modal.tsx

import { useState } from "react";

import { PanelPresence } from "@/ui/shared/component/panel.tsx";
import { closeModal, type Modal, useAskedModal } from "@/ui/shared/modal.ts";

import { SettingsModal } from "@/ui/feature/setting/modal.tsx";
import { DrawerModal } from "@/ui/feature/drawer/modal.tsx";
import { SearchModal } from "@/ui/feature/search/modal.tsx";

export function ModalHost() {
  const asked = useAskedModal();
  // A closed modal stays drawn until its panel has slid away and called
  // `onExited`.
  const [shown, setShown] = useState<Modal | null>(null);
  if (asked !== null && asked !== shown) setShown(asked);
  if (shown === null) return null;

  const onClose = closeModal;
  return (
    <PanelPresence.Provider
      value={{ open: asked !== null, onExited: () => setShown(null) }}
    >
      {shown === "settings" && <SettingsModal onClose={onClose} />}
      {shown === "drawer" && <DrawerModal onClose={onClose} />}
      {shown === "search" && <SearchModal onClose={onClose} />}
    </PanelPresence.Provider>
  );
}
