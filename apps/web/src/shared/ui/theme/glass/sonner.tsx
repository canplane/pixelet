import { cn } from "@/shared/lib/utils/cn.ts";
import { Toaster as Sonner } from "sonner";
import { TOAST_HEIGHT_PX, TOAST_STACK_PEEK_PX } from "@/shared/ui/toast.tsx";
import { BOTTOM_OVERLAY_SAFE_PX } from "@/shared/ui/overlay_safe_area.ts";

type ToasterProps = React.ComponentProps<typeof Sonner>;

const Toaster = ({ ...props }: ToasterProps) => {
  return (
    <Sonner
      position="bottom-center"
      className="toaster group"
      gap={0}
      // stretch the container to fill the screen width, centered
      style={{
        left: "50%",
        transform: "translateX(-50%)",
        width: "100%",
        display: "flex",
        /**
         * Without flexDirection, two or more toasts (e.g. an error + a
         * status) land side by side instead of stacked. Each toast already
         * disables Sonner's own absolute-position stacking (!static) and
         * relies on normal flow, so this container has to set the stack
         * axis itself — justifyContent used to do row-axis centering; that
         * role moved to alignItems once this switched to column.
         */
        flexDirection: "column",
        alignItems: "center",
        // ignore pointer events so the map underneath stays interactive
        pointerEvents: "none",
      }}
      toastOptions={{
        classNames: {
          toast: cn(
            /**
             * !static opts out of Sonner's own absolute-position stacking in
             * favor of plain flex centering. No mb-10 — margin is
             * conditional, via the <style> block below's :has(+
             * [data-sonner-toast]), since data-front alone would zero the
             * margin even with a single toast up. transform/opacity/scale
             * are left alone here and in <style>: Toast's own motion.div
             * drives those (reading stack order from useSonner()), not
             * Sonner's <li> — fighting Tailwind's !important or Sonner's own
             * stacking stylesheet for control of that never won reliably.
             */
            "group toast !bg-transparent !border-none !shadow-none !p-0 !flex !justify-center !w-full !static",
            // re-enable pointer events on the toast content only, so it stays clickable/dismissible
            "[&>div]:pointer-events-auto",
          ),
        },
      }}
      {...props}
    />
  );
};

/**
 * Margin-only now — card-stack opacity moved to Toast's own motion.div
 * (driven by useSonner()); fighting Sonner's [data-sonner-toast] <li> for
 * transform/opacity never won reliably (Tailwind's layered !important or
 * Sonner's own stacking rule, see git history for both attempts). Margin
 * has no such conflict and is also the only thing that solves the
 * "container keeps growing" bug: a transform offset is paint-only and
 * doesn't shrink the layout space Sonner's own toaster container sizes
 * around, so a full extra pill's height was reserved per stacked toast
 * (reported live as toasts drifting toward the top as more piled up).
 *
 * ":has(+ [data-sonner-toast])" ("has a sibling stacked after it") rather
 * than data-front: with one toast up, it's front but also last, and needs
 * the full breathing-room margin above the screen edge — zeroing front's
 * margin unconditionally would flush it to the bottom.
 *
 * The overlap itself comes from the adjacent-sibling rule below: a negative
 * margin-top (own height minus the desired peek) removes that space from
 * flow for real, so the container only grows by TOAST_STACK_PEEK_PX per
 * additional toast, not a full pill's height each time.
 *
 * BOTTOM_MARGIN_PX clears the floating palette bar's own footprint (see
 * overlay_safe_area.ts) so the two float as a clean stack.
 */
const BOTTOM_MARGIN_PX = BOTTOM_OVERLAY_SAFE_PX;
const STACK_STYLE = `
[data-sonner-toast] {
  margin-bottom: ${BOTTOM_MARGIN_PX}px;
  transition: margin 300ms ease;
  /**
   * Sonner's own stylesheet still has [data-sonner-toast]{transform:var(--y);
   * transition:transform .4s,...} live here — never overridden (only the
   * [data-front]/[data-expanded] rules that used to *set* --y, which do
   * nothing once this toast is position:static and stacked via margin
   * instead). --y can still shift for reasons unrelated to our layout
   * (Sonner's ResizeObserver recomputing --offset/--initial-height, its own
   * hover/expand bookkeeping), and with the transform transition still
   * armed, that quietly animates the toast a few px (reported live as a
   * nudge on hover). We already own position via margin, so transform has
   * no job left; locking it to none removes the whole class of bug instead
   * of chasing each trigger.
   */
  transform: none !important;
}
[data-sonner-toast]:has(+ [data-sonner-toast]) {
  margin-bottom: 0;
}
[data-sonner-toast] + [data-sonner-toast] {
  margin-top: -${TOAST_HEIGHT_PX - TOAST_STACK_PEEK_PX}px;
}
`;

const ToasterWithStack = (props: ToasterProps) => (
  <>
    <style>{STACK_STYLE}</style>
    <Toaster {...props} />
  </>
);

export { ToasterWithStack as Toaster };
