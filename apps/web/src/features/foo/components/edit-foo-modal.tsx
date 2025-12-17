import { useEffect, useCallback } from 'react';

import type { BaseModalProps } from '@/app/types/ui.types';

import { useAppContext } from '@/app/providers/app-context';

/**
 * Temporary dialog placeholder.
 * This will be replaced by a real dialog implementation.
 */
function Dialog(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return <></>;
}

/** UpdateFooProps */
type UpdateFooProps = {
  type: 'foo:update';
  params: { id: number };
};

/** EditFooProps */
export type EditFooProps = UpdateFooProps & {};

/**
 * EditFooModal
 *
 * A controlled modal component responsible for:
 * - Initializing modal-specific state on open
 * - Cleaning up or committing changes on close
 *
 * The modal itself does not manage visibility.
 * Visibility is controlled externally via BaseModalProps.
 */
export function EditFooModal(props: BaseModalProps & EditFooProps) {
  const { open, type, params } = props;
  const { store, ui } = useAppContext();

  /**
   * Initializes modal state when the modal opens
   */
  const initializeModal = useCallback((): void => {
    if (type === 'foo:update') {
      // Initialize state using params.id and store
    }
  }, [type, params, store]);

  /**
   * Handles dialog open state changes
   *
   * Closing the dialog triggers cleanup / initialization logic.
   */
  const handleOpenChange = (newOpen: boolean): void => {
    props.onOpenChange(false);
    // Initialize
    if (!newOpen) initializeModal();
  };

  /**
   * Run initialization when the modal becomes visible
   */
  useEffect(() => {
    if (open) initializeModal();
  }, [open]);

  return <Dialog open={open} onOpenChange={handleOpenChange} />;
}
