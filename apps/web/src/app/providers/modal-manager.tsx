import { useState, useEffect } from 'react';

import type { UiController, ModalController } from '@/app/types/ui.types';

import { EditFooModal } from '@/features/foo/components/edit-foo-modal';
import type { EditFooProps } from '@/features/foo/components/edit-foo-modal';

/**
 * Represents the currently active modal state.
 * `null` means no modal is visible.
 */
export type ModalState = EditFooProps | null;

/** ModalsProps */
type ModalsProps = {
  mergeUiController: (o: Partial<UiController>) => void; // Merges modal-related UI actions into the global UI controller
};

/**
 * Global modal host
 *
 * This component is responsible for:
 * - Managing modal visibility and state
 * - Registering imperative modal actions to the UI controller
 * - Rendering active modals at the application root level
 */
export function ModalManager(props: ModalsProps) {
  const { mergeUiController } = props;

  const [modalState, setModalState] = useState<ModalState>(null);

  /**
   * Modal-specific UI controller
   *
   * This controller exposes imperative actions
   * (e.g. opening a modal from anywhere in the app)
   */
  const [modalsController] = useState<ModalController>({
    openUpdateFooModal: (id) =>
      setModalState({ type: 'foo:update', params: { id } }),
  });

  /**
   * Register modal actions to the global UI controller
   */
  useEffect(() => {
    mergeUiController(modalsController);
  }, [modalsController]);

  // No active modal
  if (!modalState) {
    return <></>;
  }
  const { type } = modalState;

  return (
    <>
      {type === 'foo:update' && (
        <EditFooModal
          open={!!modalState}
          onOpenChange={(open) => !open && setModalState(null)}
          {...modalState}
        />
      )}
    </>
  );
}
