// @pxl/web/modules/map/ui/overlay/current_address.tsx

import { AnimatePresence, motion } from "framer-motion";

type CurrentAddressProps = {
  label: string | null;
};

/**
 * Locality-level address label for the current map center (see
 * use_reverse_geocode.ts for how it's built from Nominatim's own
 * display_name, not a hand-picked/hardcoded set of address fields). Plain
 * text only, no
 * background/padding of its own — this now sits inside the shared
 * status-bar chrome map_overlays.tsx wraps around it (alongside
 * SensorStatus, in one row) rather than floating as its own separate pill.
 * The parent also owns the useReverseGeocode() call and passes the result
 * down, so map_overlays.tsx's OSM attribution line (a Nominatim usage-policy
 * requirement, not optional styling) can key off the same `label` without a
 * second, duplicate fetch.
 */
export function CurrentAddress(props: CurrentAddressProps) {
  const { label } = props;

  return (
    <AnimatePresence mode="wait">
      {label && (
        <motion.span
          key={label}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          // The pill this sits in is w-fit and pinned to right-4, so an
          // unbounded label grows it leftward until it runs into the
          // top-left cluster. Nominatim joins three display_name segments,
          // which for a long address is well past the room available.
          // min-w-0 is required for truncation to happen at all: this is a
          // flex item, and a flex item's default min-width:auto refuses to
          // shrink below its content.
          className="text-xs font-medium text-white/70 whitespace-nowrap overflow-hidden text-ellipsis min-w-0 max-w-[40vw]"
        >
          {label}
        </motion.span>
      )}
    </AnimatePresence>
  );
}
