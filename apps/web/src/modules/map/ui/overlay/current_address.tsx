// @pxl/web/modules/map/ui/overlay/current_address.tsx

import { AnimatePresence, motion } from "framer-motion";

type CurrentAddressProps = {
  label: string | null;
};

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
          className="text-xs font-medium text-white/70 whitespace-nowrap overflow-hidden text-ellipsis min-w-0 max-w-[40vw]"
        >
          {label}
        </motion.span>
      )}
    </AnimatePresence>
  );
}
