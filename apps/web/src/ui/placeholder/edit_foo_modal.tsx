// @pxl/web/feature/manage_foo/ui/edit_foo_modal.tsx

import { useCallback, useEffect } from "react";

import type { BaseModalProps } from "@/app/type/ui.types.ts";

import { useAppContext } from "@/app/provider/app_context.ts";

/**
 * Temporary dialog placeholder.
 * This will be replaced by a real dialog implementation.
 */
function Dialog(_props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return null;
}

/** UpdateFooProps */
type UpdateFooProps = {
  type: "foo:update";
  params: { id: number };
};

/** EditFooProps — extend inline here (`UpdateFooProps & {...}`) if edit-only fields show up. */
export type EditFooProps = UpdateFooProps;

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
  const { store } = useAppContext();

  /**
   * Initializes modal state when the modal opens
   */
  const initializeModal = useCallback((): void => {
    if (type === "foo:update") {
      // initialize state using params.id and store
    }
  }, [type, params, store]);

  /**
   * Handles dialog open state changes
   *
   * Closing the dialog triggers cleanup / initialization logic.
   */
  const handleOpenChange = (newOpen: boolean): void => {
    props.onOpenChange(newOpen);

    // NOTE: close hook point (commit/cleanup) - fill later as needed.
    if (!newOpen) {
      // commit/cleanup here
    }
  };

  /**
   * Run initialization when the modal becomes visible
   */
  useEffect(() => {
    if (open) initializeModal();
  }, [open]);

  return <Dialog open={open} onOpenChange={handleOpenChange} />;
}
