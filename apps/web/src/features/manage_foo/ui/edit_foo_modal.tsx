// @pxl/web/features/manage_foo/ui/edit_foo_modal.tsx

import { useCallback, useEffect } from "react";

import type { BaseModalProps } from "@/app/types/ui.types.ts";

import { useAppContext } from "@/app/providers/app_context.ts";

/**
 * Temporary dialog placeholder.
 * This will be replaced by a real dialog implementation.
 */
function Dialog(props: {
  open: boolean;
  on_open_change: (open: boolean) => void;
}) {
  return null;
}

/** UpdateFooProps */
type UpdateFooProps = {
  type: "foo:update";
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
  const initialize_modal = useCallback((): void => {
    if (type === "foo:update") {
      // initialize state using params.id and store
    }
  }, [type, params, store]);

  /**
   * Handles dialog open state changes
   *
   * Closing the dialog triggers cleanup / initialization logic.
   */
  const handle_open_change = (newOpen: boolean): void => {
    props.on_open_change(newOpen);

    // NOTE: close hook point (commit/cleanup) - fill later as needed.
    if (!newOpen) {
      // commit/cleanup here
    }
  };

  /**
   * Run initialization when the modal becomes visible
   */
  useEffect(() => {
    if (open) initialize_modal();
  }, [open]);

  return <Dialog open={open} on_open_change={handle_open_change} />;
}
