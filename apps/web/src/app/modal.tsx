// @pxl/web/app/modal.tsx

import { useState } from "react";
import { create } from "zustand";

import { PanelPresence } from "@/shared/ui/panel.tsx";

import { SettingsModal } from "@/feature/setting/modal.tsx";
import { DrawerModal } from "@/feature/map/ui/overlay/drawer-modal.tsx";
import { SearchModal } from "@/feature/search/modal.tsx";

type Modal = "settings" | "drawer" | "search";

const useAsked = create<{ asked: Modal | null }>(() => ({ asked: null }));

export function openModal(modal: Modal): void {
  useAsked.setState({ asked: modal });
}

export function Modals() {
  const asked = useAsked((s) => s.asked);
  // Drawn until it has slid away.
  const [shown, setShown] = useState<Modal | null>(null);
  if (asked !== null && asked !== shown) setShown(asked);
  if (shown === null) return null;

  const onClose = () => useAsked.setState({ asked: null });
  return (
    <PanelPresence.Provider value={{ open: asked !== null, gone: () => setShown(null) }}>
      {shown === "settings" && <SettingsModal onClose={onClose} />}
      {shown === "drawer" && <DrawerModal onClose={onClose} />}
      {shown === "search" && <SearchModal onClose={onClose} />}
    </PanelPresence.Provider>
  );
}
