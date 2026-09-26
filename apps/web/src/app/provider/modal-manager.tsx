// @pxl/web/app/provider/modal-manager.tsx

import { useEffect, useState } from "react";

import type { ModalController, UiController } from "../type/ui.type.ts";

import { PanelPresence } from "@/shared/ui/panel.tsx";

import { SettingsModal } from "@/feature/setting/modal.tsx";
import { DrawerModal } from "@/feature/map/ui/overlay/drawer-modal.tsx";
import { SearchModal } from "@/feature/search/modal.tsx";

/** The panels a modal can be; none of them takes anything to open. */
export const enum Modal {
  Settings = 0,
  Drawer = 1,
  Search = 2,
}

/** ModalManager props */
interface ModalManagerProps {
  mergeUiController: (o: Partial<UiController>) => void; // Merges modal-related UI actions into the global UI controller
}

/**
 * Global modal host
 *
 * This component is responsible for:
 * - Managing modal visibility and state
 * - Registering imperative modal actions to the UI controller
 * - Rendering active modals at the application root level
 */
export function ModalManager(props: ModalManagerProps) {
  const { mergeUiController } = props;

  /** The modal asked for; null when none is. */
  const [asked, setAsked] = useState<Modal | null>(null);

  /**
   * Modal-specific UI controller
   *
   * This controller exposes imperative actions
   * (e.g. opening a modal from anywhere in the app)
   */
  const [modalsController] = useState<ModalController>({
    openSettingsModal: () => setAsked(Modal.Settings),
    openDrawerModal: () => setAsked(Modal.Drawer),
    openSearchModal: () => setAsked(Modal.Search),
  });

  /**
   * Register modal actions to the global UI controller
   */
  useEffect(() => {
    mergeUiController(modalsController);
  }, [modalsController, mergeUiController]);

  // What is drawn: the modal asked for, or the last one while it slides
  // away. A modal asked for over one still leaving replaces it at once.
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
