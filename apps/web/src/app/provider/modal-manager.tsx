// @pxl/web/app/provider/modal-manager.tsx

import { useEffect, useState } from "react";

import type { ModalController, UiController } from "../type/ui.type.ts";

import { PanelPresence } from "@/shared/ui/panel.tsx";

import { SettingsModal } from "@/feature/setting/modal.tsx";
import { DrawerModal } from "@/feature/map/ui/overlay/drawer-modal.tsx";
import { SearchModal } from "@/feature/search/modal.tsx";

export const enum Modal {
  Settings = 0,
  Drawer = 1,
  Search = 2,
}

interface ModalManagerProps {
  mergeUiController: (o: Partial<UiController>) => void;
}

export function ModalManager(props: ModalManagerProps) {
  const { mergeUiController } = props;

  const [asked, setAsked] = useState<Modal | null>(null);

  const [modalsController] = useState<ModalController>({
    openSettingsModal: () => setAsked(Modal.Settings),
    openDrawerModal: () => setAsked(Modal.Drawer),
    openSearchModal: () => setAsked(Modal.Search),
  });

  useEffect(() => {
    mergeUiController(modalsController);
  }, [modalsController, mergeUiController]);

  // The last modal stays drawn while it slides away; a new one replaces it at once.
  const [shown, setShown] = useState<Modal | null>(null);
  if (asked !== null && asked !== shown) setShown(asked);

  if (shown === null) {
    return null;
  }
  const onClose = () => setAsked(null);

  return (
    <PanelPresence.Provider value={{ open: asked !== null, gone: () => setShown(null) }}>
      {shown === Modal.Settings && <SettingsModal onClose={onClose} />}
      {shown === Modal.Drawer && <DrawerModal onClose={onClose} />}
      {shown === Modal.Search && <SearchModal onClose={onClose} />}
    </PanelPresence.Provider>
  );
}
