// @pxl/web/app/providers/modal-manager.ts

import { useEffect, useState } from "react";

import type { ModalController, UIController } from "../types/ui.types.ts";

import { EditFooModal } from "@/features/manage-foo/ui/edit-foo-modal.tsx";
import type { EditFooProps } from "@/features/manage-foo/ui/edit-foo-modal.tsx";

/**
 * Represents the currently active modal state.
 * `null` means no modal is visible.
 */
export type ModalState = EditFooProps | null;

/** ModalManagerProps */
type ModalManagerProps = {
  mergeUIController: (o: Partial<UIController>) => void; // Merges modal-related UI actions into the global UI controller
};

/**
 * Global modal host
 *
 * This component is responsible for:
 * - Managing modal visibility and state
 * - Registering imperative modal actions to the UI controller
 * - Rendering active modals at the application root level
 */
export function ModalManager(props: ModalManagerProps) {
  const { mergeUIController } = props;

  const [modalState, setModalState] = useState<ModalState>(null);

  /**
   * Modal-specific UI controller
   *
   * This controller exposes imperative actions
   * (e.g. opening a modal from anywhere in the app)
   */
  const [modalsController] = useState<ModalController>({
    openUpdateFooModal: (id) => setModalState({ type: "foo:update", params: { id } }),
  });

  /**
   * Register modal actions to the global UI controller
   */
  useEffect(() => {
    mergeUIController(modalsController);
  }, [modalsController, mergeUIController]);

  // No active modal
  if (!modalState) {
    return null;
  }
  const { type } = modalState;

  return (
    <>
      {type === "foo:update" && (
        <EditFooModal
          open={!!modalState}
          onOpenChange={(open) => !open && setModalState(null)}
          {...modalState}
        />
      )}
    </>
  );
}
