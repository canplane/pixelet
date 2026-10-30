// @pxl/client/shell/modal.tsx

import { useState } from "react";
import { create } from "zustand";

import { PanelPresence } from "@/shell/ui/panel.tsx";

import { SettingsModal } from "@/shell/setting/modal.tsx";
import { DrawerModal } from "@/shell/drawer/modal.tsx";
import { SearchModal } from "@/shell/search/modal.tsx";

type Modal = "settings" | "drawer" | "search";

const store = create<{ asked: Modal | null }>()(() => ({ asked: null }));

export function openModal(modal: Modal): void {
  store.setState({ asked: modal });
}

export function closeModal(): void {
  store.setState({ asked: null });
}

export function modalOpen(): boolean {
  return store.getState().asked !== null;
}

export function Modals() {
  const asked = store((s) => s.asked);
  // Drawn until it has slid away.
  const [shown, setShown] = useState<Modal | null>(null);
  if (asked !== null && asked !== shown) setShown(asked);
  if (shown === null) return null;

  const onClose = closeModal;
  return (
    <PanelPresence.Provider
      value={{ open: asked !== null, gone: () => setShown(null) }}
    >
      {shown === "settings" && <SettingsModal onClose={onClose} />}
      {shown === "drawer" && <DrawerModal onClose={onClose} />}
      {shown === "search" && <SearchModal onClose={onClose} />}
    </PanelPresence.Provider>
  );
}
